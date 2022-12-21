pub use once_cell::sync::OnceCell;
pub use tokio::sync::Mutex;
pub use wasm_bindgen_futures::spawn_local;

#[macro_export]
macro_rules! build_perform {
    ($space:ident, $key:ty, $value:ty) => {
        mod $space {
            pub mod perform {
                use std::collections::HashMap;
                use std::future::Future;
                use std::hash::Hash;
                static STORE: $crate::OnceCell<$crate::Mutex<HashMap<$key, $value>>> =
                    $crate::OnceCell::new();

                fn global_data() -> &'static $crate::Mutex<HashMap<$key, $value>>
                where
                    $key: Hash,
                {
                    STORE.get_or_init(|| {
                        let hash_map = HashMap::new();
                        $crate::Mutex::new(hash_map)
                    })
                }

                async fn lock_and_do_mut<F>(f: F) -> Option<$value>
                where
                    F: FnOnce(&mut HashMap<$key, $value>) -> Option<$value>,
                    $key: Hash,
                {
                    let mut hash_map = global_data().lock().await;
                    f(&mut *hash_map)
                }

                async fn lock_and_do<F>(f: F) -> Option<$value>
                where
                    F: Fn(&HashMap<$key, $value>) -> Option<$value>,
                {
                    let hash_map = global_data().lock().await;
                    f(&*hash_map)
                }
                #[allow(dead_code)]
                fn try_lock_and_do<F>(f: F) -> Option<$value>
                where
                    F: Fn(&HashMap<$key, $value>) -> Option<$value>,
                {
                    let hash_map = global_data().try_lock().ok()?;
                    f(&*hash_map)
                }

                fn get_and_clone(key: $key, hash_map: &HashMap<$key, $value>) -> Option<$value>
                where
                    $value: Clone,
                {
                    hash_map.get(&key).map(|v| v.clone())
                }

                pub async fn set_async<Fut>(key: $key, f: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let value = f.await;
                    lock_and_do_mut(|hash_map| hash_map.insert(key, value)).await;
                }
                #[allow(dead_code)]
                pub fn set_begin<Fut>(key: $key, f: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    $crate::spawn_local(async move {
                        let value = f.await;
                        lock_and_do_mut(|hash_map| hash_map.insert(key, value)).await;
                    });
                }
                pub async fn fetch_async(key: $key) -> Option<$value> {
                    lock_and_do(|hash_map| get_and_clone(key, hash_map)).await
                }
                #[allow(dead_code)]
                pub fn try_fetch(key: $key) -> Option<$value> {
                    let value = try_lock_and_do(|hash_map| get_and_clone(key, hash_map))?;
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
        assert!(test_module::perform::fetch_async(1).await.is_none());
        test_module::perform::set_async(1, async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        })
        .await;
        let body = test_module::perform::fetch_async(1).await;
        log::debug!("body: {:?}", body);
        assert!(body.is_some());
        assert!(body.unwrap().contains("origin"));
    }
}
