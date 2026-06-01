fn main() {
    println!("hello");
}

pub struct MyStruct {
    pub name: String,
}

impl MyStruct {
    pub fn new() -> Self {
        MyStruct { name: "test".into() }
    }
}
