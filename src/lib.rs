pub use once_cell::sync::OnceCell;
pub use thiserror::Error;
pub use tokio::sync::Mutex;
pub use uuid::Uuid;
pub use wasm_bindgen_futures::spawn_local;

pub struct Session {
    id: Uuid,
}

#[derive(Debug, Error)]
pub enum PerformError {
    #[error("NotInitialized")]
    NotInitialized,
    #[error("Locked")]
    Locked,
}

pub enum PerformState<T> {
    Empty,
    Done(T),
}

#[macro_export]
macro_rules! build_perform {
    ($space:ident, $value:ty) => {
        mod $space {
            use std::collections::HashMap;
            use std::future::Future;
            use std::hash::Hash;
            type V = $value;
            type H = HashMap<$crate::Uuid, Result<V, $crate::PerformError>>;

            static STORE: $crate::OnceCell<$crate::Mutex<H>> = $crate::OnceCell::new();

            fn global_data() -> &'static $crate::Mutex<H> {
                STORE.get_or_init(|| {
                    let hash_map = HashMap::new();
                    $crate::Mutex::new(hash_map)
                })
            }

            async fn lock_and_do_mut<F, R>(f: F) -> R
            where
                F: FnOnce(&mut H) -> R,
            {
                let mut hash_map = global_data().lock().await;
                f(&mut *hash_map)
            }
            async fn lock_and_do<F, R>(f: F) -> R
            where
                F: Fn(&H) -> R,
            {
                let hash_map = global_data().lock().await;
                f(&*hash_map)
            }

            #[allow(dead_code)]
            fn try_lock_and_do_mut<F>(f: F) -> Result<V, $crate::PerformError>
            where
                F: FnOnce(&mut H) -> Result<V, $crate::PerformError>,
            {
                let mut try_lock = global_data().try_lock();
                match try_lock {
                    Ok(mut hash_map) => f(&mut *hash_map),
                    Err(_) => Err($crate::PerformError::Locked),
                }
            }
            #[allow(dead_code)]
            fn try_lock_and_do<F>(f: F) -> Result<V, $crate::PerformError>
            where
                F: Fn(&H) -> Result<V, $crate::PerformError>,
            {
                let try_lock = global_data().try_lock();
                match try_lock {
                    Ok(hash_map) => f(&*hash_map),
                    Err(_) => Err($crate::PerformError::Locked),
                }
            }

            impl $crate::Session {
                pub async fn activate() -> Self {
                    let id = $crate::Uuid::new_v4();
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(id, Err($crate::PerformError::Empty))
                    })
                    .await;
                    Self { id }
                }
                pub fn activate_with_spawn_local() -> Self {
                    let id = $crate::Uuid::new_v4();
                    $crate::spawn_local(async move {
                        lock_and_do_mut(|hash_map| {
                            hash_map.insert(id, Err($crate::PerformError::Empty));
                        })
                        .await;
                    });
                    Self { id }
                }

                pub async fn perform<Fut>(&self, fut: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let value = fut.await;
                    lock_and_do_mut(|hash_map| hash_map.insert(self.id, Ok(value))).await;
                }
                #[allow(dead_code)]
                pub fn perform_with_spawn_local<Fut>(&self, fut: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let id = self.id.clone();
                    $crate::spawn_local(async move {
                        let value = fut.await;
                        lock_and_do_mut(|hash_map| hash_map.insert(id, Ok(value))).await;
                    });
                }

                pub async fn clone(&self) -> Result<V, $crate::PerformError> {
                    lock_and_do(|hash_map| {
                        let some_result: Option<&Result<V, $crate::PerformError>> =
                            hash_map.get(&self.id);
                        match some_result {
                            Some(result) => match result {
                                Ok(v) => Ok(v.clone()),
                                Err(_) => Err($crate::PerformError::Empty),
                            },
                            None => Err($crate::PerformError::NotInitialized),
                        }
                    })
                    .await
                }
                pub async fn take(&self) -> Result<V, $crate::PerformError> {
                    lock_and_do_mut(|hash_map| {
                        let some_result: Option<Result<V, $crate::PerformError>> =
                            hash_map.remove_entry(&self.id).map(|(_id, r)| r);
                        match some_result {
                            Some(result) => result.map_err(|_| $crate::PerformError::Empty),
                            None => Err($crate::PerformError::NotInitialized),
                        }
                    })
                    .await
                }

                #[allow(dead_code)]
                pub fn try_clone(&self) -> Result<V, $crate::PerformError> {
                    let some_result = try_lock_and_do(|hash_map| {
                        let optional = hash_map.get(&self.id);
                        let result: Result<V, $crate::PerformError> = match optional {
                            None => Err($crate::PerformError::NotInitialized),
                            Some(&result) => match result {
                                Err($crate::PerformError::Empty) => {
                                    Err($crate::PerformError::Empty)
                                }
                            },
                        };
                    });
                    match some_result {
                        Some(result) => result,
                        None => Err($crate::PerformError::Locked),
                    }
                }
                #[allow(dead_code)]
                pub fn try_take(&self) -> Result<V, $crate::PerformError> {
                    let optional_result = try_lock_and_do_mut(|hash_map| {
                        let some_result: Option<Result<V, $crate::PerformError>> =
                            hash_map.remove_entry(&self.id).map(|(_id, r)| r);
                        match some_result {
                            Some(result) => result.map_err(|_| $crate::PerformError::Empty),
                            None => Err($crate::PerformError::NotInitialized),
                        }
                    });
                    match optional_result {
                        Some(result) => result,
                        None => Err($crate::PerformError::Locked),
                    }
                }

                // fn get<'a>(&self, hash_map: &'a H) -> Option<&'a V> {
                //     hash_map.get(&self.id).map(|v| v.as_ref().ok()).flatten()
                // }
                // fn get_and_clone(&self, hash_map: &H) -> Option<V>
                // where
                //     V: Clone,
                // {
                //     match hash_map.get(&self.id) {
                //         Some(r) => match r {
                //             Ok(v) => Some(v.clone()),
                //             Err(_) => None,
                //         },
                //         None => None,
                //     }
                // }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    wasm_bindgen_test_configure!(run_in_browser);

    build_perform!(test_module, String);
    use crate::Session;
    use test_module::*;

    #[test]
    fn first_test() {
        assert!(true);
    }

    #[wasm_bindgen_test]
    async fn second_test() -> anyhow::Result<()> {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::default());
        log::trace!("some trace log");
        log::debug!("some debug log");
        log::info!("some info log");
        log::warn!("some warn log");
        log::error!("some error log");

        let fut = async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        let id = Session::activate().await;
        id.perform(fut).await;
        // let ip_result = test_module::peek(id).await;
        // assert!(ip_result.is_ok());
        // let ip = ip_result?;
        // assert!(ip.contains("origin"));
        let ip_result = id.take().await;
        assert!(ip_result.is_ok());
        let ip = ip_result?;
        assert!(ip.contains("origin"));
        let ip_result = id.take().await;
        assert!(ip_result.is_err());
        Ok(())
    }
}
