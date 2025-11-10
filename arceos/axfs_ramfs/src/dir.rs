use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};
use crate::alloc::string::ToString;
use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType};
use axfs_vfs::{VfsError, VfsResult};
use spin::RwLock;

use crate::file::FileNode;

/// The directory node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct DirNode {
    this: Weak<DirNode>,
    parent: RwLock<Weak<dyn VfsNodeOps>>,
    children: RwLock<BTreeMap<String, VfsNodeRef>>,//目录项表
    //                        ↑文件名    ↑文件实体（相当于inode的抽象
    //BTreeMap数据结构相当于一个map 存键值对 但是将键值对插到平衡二叉树 内部是有序的并且迭代的时候按照键的顺序
}

impl DirNode {
    pub(super) fn new(parent: Option<Weak<dyn VfsNodeOps>>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            parent: RwLock::new(parent.unwrap_or_else(|| Weak::<Self>::new())),
            children: RwLock::new(BTreeMap::new()),
        })
    }

    pub(super) fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// Returns a string list of all entries in this directory.
    pub fn get_entries(&self) -> Vec<String> {
        self.children.read().keys().cloned().collect()
    }

    /// Checks whether a node with the given name exists in this directory.
    pub fn exist(&self, name: &str) -> bool {
        self.children.read().contains_key(name)
    }

    /// Creates a new node with the given name and type in this directory.
    pub fn create_node(&self, name: &str, ty: VfsNodeType) -> VfsResult {
        if self.exist(name) {
            log::error!("AlreadyExists {}", name);
            return Err(VfsError::AlreadyExists);
        }
        let node: VfsNodeRef = match ty {
            VfsNodeType::File => Arc::new(FileNode::new()),
            VfsNodeType::Dir => Self::new(Some(self.this.clone())),
            _ => return Err(VfsError::Unsupported),
        };
        self.children.write().insert(name.into(), node);
        Ok(())
    }

    /// Removes a node by the given name in this directory.
    pub fn remove_node(&self, name: &str) -> VfsResult {
        let mut children = self.children.write();
        let node = children.get(name).ok_or(VfsError::NotFound)?;
        if let Some(dir) = node.as_any().downcast_ref::<DirNode>() {
            if !dir.children.read().is_empty() {
                return Err(VfsError::DirectoryNotEmpty);
            }
        }
        children.remove(name);
        Ok(())
    }
    fn rename_in_same_dir(&self, old_name: &str, new_name: &str) -> VfsResult {
        log::error!("[dir] call rename_in_same_dir oldname:{},newname:{}",old_name,new_name);
        let mut children = self.children.write();  // Rwlock是读写锁 通过.write（） 获取写锁
        //可以有多个读锁（共享） 或者有一个写锁（独占） 但不能同时有读锁和写锁
        // 现在可以安全地修改 children...
        log::error!("[dir] getting node");
        let node=children.get(old_name).ok_or(VfsError::NotFound)?.clone();//找到old_name对应inode
        //get是通过键查找值 返回option  ok_or(E)将option<T>转化为Result<T,E>
        // ?操作符可以跟在option或者result后面 成功了就解包 失败了就提前返回
         // 检查目标是否已存在
        if children.contains_key(new_name) {
            return Err(VfsError::AlreadyExists);
        }
         // 关键操作：修改名称映射，不复制数据！
        children.remove(old_name);
        children.insert(new_name.to_string(),node);// 同一个 node 对象！

        log::error!("[dir] complete rename in same dir!");
        // VfsResult的定义 等价于：type VfsResult<T> = Result<T, VfsError>;
        Ok(())
    }
}

impl VfsNodeOps for DirNode {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_dir(4096, 0))
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        self.parent.read().upgrade()
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let (name, rest) = split_path(path);
        let node = match name {
            "" | "." => Ok(self.clone() as VfsNodeRef),
            ".." => self.parent().ok_or(VfsError::NotFound),
            _ => self
                .children
                .read()
                .get(name)
                .cloned()
                .ok_or(VfsError::NotFound),
        }?;

        if let Some(rest) = rest {
            node.lookup(rest)
        } else {
            Ok(node)
        }
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        let children = self.children.read();
        let mut children = children.iter().skip(start_idx.max(2) - 2);
        for (i, ent) in dirents.iter_mut().enumerate() {
            match i + start_idx {
                0 => *ent = VfsDirEntry::new(".", VfsNodeType::Dir),
                1 => *ent = VfsDirEntry::new("..", VfsNodeType::Dir),
                _ => {
                    if let Some((name, node)) = children.next() {
                        *ent = VfsDirEntry::new(name, node.get_attr().unwrap().file_type());
                    } else {
                        return Ok(i);
                    }
                }
            }
        }
        Ok(dirents.len())
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        log::debug!("create {:?} at ramfs: {}", ty, path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.create(rest, ty),
                ".." => self.parent().ok_or(VfsError::NotFound)?.create(rest, ty),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.create(rest, ty)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Ok(()) // already exists
        } else {
            self.create_node(name, ty)
        }
    }

    fn remove(&self, path: &str) -> VfsResult {
        log::debug!("remove at ramfs: {}", path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.remove(rest),
                ".." => self.parent().ok_or(VfsError::NotFound)?.remove(rest),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.remove(rest)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Err(VfsError::InvalidInput) // remove '.' or '..
        } else {
            self.remove_node(name)
        }
    }
    
    fn rename(&self,src_path:&str,dst_path:&str)->VfsResult{
        log::error!("[dir] call rename");
        //现在先实现只支持在同一目录下重命名
        let (src_name, src_rest) = split_path(src_path);
        let (dst_dir,dst_rest)=split_path(dst_path);
        let dst_rest=dst_rest.unwrap();
        //
        
        self.rename_in_same_dir(src_name, dst_rest)
    }
    axfs_vfs::impl_vfs_dir_default! {}//提供了其他方法的默认实现
    //经过查cargo.lock 发现axfs_vfs是外部依赖
}

fn split_path(path: &str) -> (&str, Option<&str>) {//去掉开头的/  再以中间的第一个/进行切分
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}
