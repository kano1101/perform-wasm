pub use once_cell::sync::OnceCell;
pub use thiserror::Error;
pub use tokio::sync::Mutex;
pub use uuid::Uuid;
pub use wasm_bindgen_futures::spawn_local;

pub struct Session {
    id: Uuid,
}

#[derive(Debug, Error, Clone)]
pub enum PerformError {
    #[error("NotSecured")]
    NotSecured,
    #[error("Locked")]
    Locked,
}

#[derive(Clone)]
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
            type V = $crate::PerformState<$value>;
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
            fn try_lock_and_do_mut<F>(f: F) -> Result<V, $crate::PerformError>
            where
                F: FnOnce(&mut H) -> Result<V, $crate::PerformError>,
            {
                let try_lock = global_data().try_lock();
                match try_lock {
                    Ok(mut hash_map) => f(&mut *hash_map),
                    Err(_) => Err($crate::PerformError::Locked),
                }
            }

            impl $crate::Session {
                #[cfg(not(target_arch = "wasm32"))]
                pub async fn activate() -> Self {
                    let id = $crate::Uuid::new_v4();
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(id, Ok($crate::PerformState::Empty))
                    })
                    .await;
                    Self { id }
                }
                #[cfg(target_arch = "wasm32")]
                pub fn activate_with_spawn_local() -> Self {
                    let id = $crate::Uuid::new_v4();
                    $crate::spawn_local(async move {
                        lock_and_do_mut(|hash_map| {
                            hash_map.insert(id, Ok($crate::PerformState::Empty));
                        })
                        .await;
                    });
                    Self { id }
                }

                #[cfg(not(target_arch = "wasm32"))]
                pub async fn perform<Fut>(&self, fut: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let value = fut.await;
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(self.id, Ok($crate::PerformState::Done(value)))
                    })
                    .await;
                }
                #[cfg(target_arch = "wasm32")]
                pub fn perform_with_spawn_local<Fut>(&self, fut: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let id = self.id.clone();
                    $crate::spawn_local(async move {
                        let value = fut.await;
                        lock_and_do_mut(|hash_map| {
                            hash_map.insert(id, Ok($crate::PerformState::Done(value)))
                        })
                        .await;
                    });
                }

                #[cfg(not(target_arch = "wasm32"))]
                pub async fn take(&self) -> Result<V, $crate::PerformError> {
                    lock_and_do_mut(|hash_map| Self::take_from_id(hash_map, &self.id)).await
                }

                pub fn try_take(&self) -> Result<V, $crate::PerformError> {
                    try_lock_and_do_mut(|hash_map| Self::take_from_id(hash_map, &self.id))
                }

                fn get_as_take(
                    hash_map: &mut H,
                    id: &$crate::Uuid,
                ) -> Option<Result<$crate::PerformState<String>, $crate::PerformError>> {
                    hash_map.remove_entry(id).map(|(_id, r)| r)
                }
                fn into_as_take<T, E>(result: Result<T, E>) -> Result<T, E> {
                    result
                }
                fn take_from_id(
                    hash_map: &mut H,
                    id: &$crate::Uuid,
                ) -> Result<V, $crate::PerformError> {
                    let some_result = Self::get_as_take(hash_map, id);
                    match some_result {
                        Some(result) => Self::into_as_take(result),
                        None => Err($crate::PerformError::NotSecured),
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use crate::{PerformState, Session};

    async fn run_test<Fut, T, A>(fut: Fut, assert: A)
    where
        Fut: std::future::Future<Output = T> + 'static,
        A: FnOnce(T),
    {
        let session = Session::activate().await;
        session.perform(fut).await;

        let value_result = session.take().await;
        assert!(value_result.is_ok());

        if let PerformState::Done(value) = value_result? {
            assert(value);
        } else {
            assert!(false);
        }

        let value_result = session.take().await;
        assert!(value_result.is_err());
    }

    #[test]
    fn first_test() {
        assert!(true);
    }

    build_perform!(namespace_ip, String);
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn second_test() -> anyhow::Result<()> {
        let fut = async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        let assert = |text| {
            assert!(text.contains("origin"));
        };
        run_test(fut, assert).await;
        log::debug!("成功しました。");

        assert!(false);
        Ok(())
    }

    build_perform!(namespace_status, reqwest::StatusCode);
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    async fn third_test() -> anyhow::Result<()> {
        console_error_panic_hook::set_once();
        #[cfg(target_arch = "wasm32")]
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
                .status()
        };
        let assert = |staus| {
            assert_eq!(status == reqwest::StatusCode::OK);
        };
        run_test(fut, assert).await;
        log::debug!("成功しました。");

        assert!(false);
        Ok(())
    }
}
