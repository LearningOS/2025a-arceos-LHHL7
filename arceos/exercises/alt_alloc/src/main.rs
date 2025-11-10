#![cfg_attr(feature = "axstd", no_std)]
//如果启用了 axstd 特性，那么应用 #![no_std] 属性" 即不使用rust 标准库
#![cfg_attr(feature = "axstd", no_main)]//不使用标准main

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;
extern crate alloc;

use alloc::vec::Vec;

#[cfg_attr(feature = "axstd", no_mangle)]//禁止rust名称修饰
fn main() {
    println!("Running bump tests...");

    #[cfg(feature = "axstd")]
    println!("axstd feature is ENABLED");
    
    #[cfg(not(feature = "axstd"))]
    println!("axstd feature is DISABLED");
    const N: usize = 3_000_000;
    let mut v = Vec::with_capacity(N);
    for i in 0..N {
        v.push(i);
    }
    v.sort();
    for i in 0..N - 1 {
        assert!(v[i] <= v[i + 1]);
    }

    println!("Bump tests run OK!");
}
