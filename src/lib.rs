#[macro_export]
macro_rules! build_perform {
    ($space:ident, $key:ty, $value:ty) => {
        mod $space {
            pub mod perform {
                use once_cell::sync::OnceCell;
                use std::collections::HashMap;
                use std::future::Future;
                use std::hash::Hash;
                use tokio::sync::Mutex;
                use wasm_bindgen_futures::spawn_local;
                static STORE: OnceCell<Mutex<HashMap<$key, $value>>> = OnceCell::new();

                fn global_data() -> &'static Mutex<HashMap<$key, $value>>
                where
                    $key: Hash,
                    $value: Default,
                {
                    STORE.get_or_init(|| {
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

                async fn lock_and_pop<F>(f: F) -> Option<$value>
                where
                    F: Fn(&HashMap<$key, $value>) -> Option<$value>,
                {
                    let hash_map = global_data().lock().await;
                    f(&*hash_map)
                }
                #[allow(dead_code)]
                fn lock_and_pop_wasm<F>(f: F) -> Option<$value>
                where
                    F: Fn(&HashMap<$key, $value>) -> Option<$value>,
                {
                    let hash_map = global_data().try_lock().ok()?;
                    f(&*hash_map)
                }

                async fn lock_and_insert(key: $key, value: $value) {
                    lock_and_push(|hash_map| hash_map.insert(key, value)).await;
                }
                fn get_and_clone(key: $key, hash_map: &HashMap<$key, $value>) -> Option<$value>
                where
                    $value: Clone,
                {
                    hash_map.get(&key).map(|v| v.clone())
                }

                pub async fn push<Fut>(key: $key, f: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let value = f.await;
                    lock_and_insert(key, value).await;
                }
                #[allow(dead_code)]
                pub fn push_wasm<Fut>(key: $key, f: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    spawn_local(async move {
                        let value = f.await;
                        lock_and_insert(key, value).await;
                    });
                }
                pub async fn pop(key: $key) -> Option<$value> {
                    lock_and_pop(|hash_map| get_and_clone(key, hash_map)).await
                }
                #[allow(dead_code)]
                pub fn pop_wasm(key: $key) -> Option<$value> {
                    let value = lock_and_pop_wasm(|hash_map| get_and_clone(key, hash_map))?;
                    Some(value)
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    wasm_bindgen_test_configure!(run_in_browser);

    build_perform!(test_module, i32, String);

    #[test]
    fn first_test() {
        assert!(true);
    }

    #[wasm_bindgen_test]
    async fn second_test() {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::default());
        log::trace!("some trace log");
        log::debug!("some debug log");
        log::info!("some info log");
        log::warn!("some warn log");
        log::error!("some error log");
        assert!(test_module::perform::pop(1).await.is_none());
        test_module::perform::push(1, async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        })
        .await;
        let body = test_module::perform::pop(1).await;
        log::debug!("body: {:?}", body);
        assert!(body.is_some());
        assert!(body.unwrap().contains("origin"));
    }
}
