macro_rules! build_perform {
    ($space:ident, $key:ty, $value:ty) => {
        mod $space {
            pub mod perform {
                use core::future::Future;
                use once_cell::sync::OnceCell;
                use std::collections::HashMap;
                use std::hash::Hash;
                use tokio::sync::Mutex;
                use wasm_bindgen_futures::spawn_local;
                static A: OnceCell<Mutex<HashMap<$key, $value>>> = OnceCell::new();

                fn global_data() -> &'static Mutex<HashMap<$key, $value>>
                where
                    $key: Hash,
                    $value: Default,
                {
                    A.get_or_init(|| {
                        let hash_map = HashMap::new();
                        Mutex::new(hash_map)
                    })
                }
                async fn lock_and_push<F>(f: F) -> Option<$value>
                where
                    F: FnOnce(&mut HashMap<$key, $value>) -> Option<$value>,
                    $key: Hash,
                {
                    let mut hash_map = global_data().lock().await;
                    f(&mut *hash_map)
                }
                fn lock_and_pop<F>(f: F) -> Option<$value>
                where
                    F: Fn(&HashMap<$key, $value>) -> Option<$value>,
                {
                    let hash_map = global_data().try_lock().unwrap();
                    f(&*hash_map)
                }
                pub fn push<Fut>(key: $key, f: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    spawn_local(async move {
                        let output = f.await;
                        lock_and_push(|hash_map| hash_map.insert(key, output)).await;
                    });
                }
                pub fn pop(key: $key) -> Option<$value>
                where
                    $value: Clone,
                {
                    lock_and_pop(|hash_map| hash_map.get(&key).map(|v| v.clone()))
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    build_perform!(test_module, i32, String);
    #[test]
    fn it_works() {
        assert!(true);
    }
}
