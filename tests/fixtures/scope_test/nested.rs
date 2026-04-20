mod outer_mod {
    pub struct OuterStruct {
        value: i32,
    }

    impl OuterStruct {
        pub fn outer_method(&self) -> i32 {
            let inner_fn = || self.value + 1;
            inner_fn()
        }

        pub fn deep_method(&self) -> bool {
            self.value > 0
        }
    }

    pub struct InnerStruct {
        name: String,
    }

    impl InnerStruct {
        pub fn inner_method(&self) -> &str {
            &self.name
        }
    }
}
