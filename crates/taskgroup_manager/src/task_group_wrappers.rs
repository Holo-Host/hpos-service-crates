//! Convenience wrappers around TaskGroup and TaskManager

/// Wrappers around TaskGroup and TaskManager that panic if any of the spawned tasks panic.
pub mod propagate_panics {
    use std::{any::Any, fmt::Display, future::Future, pin::Pin, task::Poll};

    pub struct TaskGroup<E>(task_group::TaskGroup<E>);
    pub struct TaskManager<E>(task_group::TaskManager<E>);

    impl<E: Send + 'static> TaskGroup<E> {
        pub fn new() -> (TaskGroup<E>, TaskManager<E>) {
            let (task_group, task_manager) = task_group::TaskGroup::new();
            (TaskGroup(task_group), TaskManager(task_manager))
        }

        pub fn spawn<'f>(
            &'f self,
            name: &'f str,
            fut: impl Future<Output = Result<(), E>> + Send + 'static,
        ) -> impl Future<Output = ()> + Send + 'f {
            // Box the future. This limits the size in memory of the parent task.
            // Without this, the parent future would include enough space for the child future,
            // which can lead to stack overflows from too much memory usage.
            //
            // Upstream issue: https://github.com/pchickey/task-group/issues/3
            let fut = Box::pin(fut);

            async move {
                // Ignore failures to spawn. It means that the TaskManager has errored/panicked.
                // We assume that the caller will await the TaskManager to learn about that.
                let _result = self.0.spawn(name, fut).await;
            }
        }
    }

    impl<E> Future for TaskManager<E> {
        type Output = Result<(), E>;

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Self::Output> {
            match task_group::TaskManager::poll(Pin::new(&mut self.0), cx) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(task_group::RuntimeError::Panic { name, panic })) => {
                    panic!("task {:?} panicked with {}", name, PanicPayload(panic))
                }
                Poll::Ready(Err(task_group::RuntimeError::Application {
                    // We assume that the caller uses snafu to attach whatever
                    // context they need to their errors, so they don't need the name.
                    name: _,
                    error,
                })) => Poll::Ready(Err(error)),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    impl<E> Clone for TaskGroup<E> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    pub struct PanicPayload(pub Box<dyn Any>);

    impl Display for PanicPayload {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let panic_message = if let Some(s) = self.as_formatted_panic() {
                s
            } else if let Some(s) = self.as_literal_panic() {
                s
            } else {
                return f.write_str("[unprintable panic payload]");
            };
            write!(f, "{:?}", panic_message)
        }
    }

    impl PanicPayload {
        /// If the panic contained a format string, returns the formatted panic message. Otherwise returns None.
        ///
        /// Example
        /// ```
        /// # use std::{any::Any, panic::catch_unwind};
        /// # use taskgroup_manager::task_group_wrappers::propagate_panics::PanicPayload;
        /// let result: Result<(), Box<dyn Any + Send>> = catch_unwind(|| panic!("fatal: {}", "problem"));
        /// let panic_payload = PanicPayload(result.unwrap_err());
        /// // assert_eq!(panic_payload.as_formatted_panic(), Some("fatal: problem"));
        /// ```
        pub fn as_formatted_panic(&self) -> Option<&str> {
            self.0.downcast_ref::<String>().map(|string| &**string)
        }

        /// If the panic message was simply a constant string, returns it. Otherwise returns None.
        ///
        /// Example
        /// ```
        /// # use std::{any::Any, panic::catch_unwind};
        /// # use taskgroup_manager::task_group_wrappers::propagate_panics::PanicPayload;
        /// let result: Result<(), Box<dyn Any + Send>> = catch_unwind(|| panic!("static panic message"));
        /// let panic_payload = PanicPayload(result.unwrap_err());
        /// // assert_eq!(panic_payload.as_literal_panic(), Some("static panic message"));
        /// ```
        pub fn as_literal_panic(&self) -> Option<&str> {
            self.0.downcast_ref::<&str>().map(|string| &**string)
        }
    }
}

/// Wrappers around TaskGroup and TaskManager for when the spawned tasks cannot error.
pub mod infallible {
    use std::{future::Future, pin::Pin, task::Poll};

    use super::propagate_panics;

    #[derive(Clone)]
    pub struct TaskGroup(propagate_panics::TaskGroup<Infallible>);

    pub struct TaskManager(propagate_panics::TaskManager<Infallible>);

    enum Infallible {}

    impl TaskGroup {
        pub fn new() -> (TaskGroup, TaskManager) {
            let (task_group, task_manager) = propagate_panics::TaskGroup::new();
            (TaskGroup(task_group), TaskManager(task_manager))
        }

        pub fn spawn<'f>(
            &'f self,
            name: &'f str,
            fut: impl Future<Output = ()> + Send + 'static,
        ) -> impl Future<Output = ()> + Send + 'f {
            self.0.spawn(name, async move {
                fut.await;
                Ok(())
            })
        }
    }

    impl Future for TaskManager {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Self::Output> {
            match propagate_panics::TaskManager::poll(Pin::new(&mut self.0), cx) {
                Poll::Ready(Ok(())) => Poll::Ready(()),
                Poll::Ready(Err(error)) => match error {}, // This branch is unreachable
                Poll::Pending => Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod test {

    #[tokio::test]
    #[should_panic = r#"task "task name" panicked with "panic message""#]
    async fn it_propagates_panics() {
        let (task_group, task_manager) = super::infallible::TaskGroup::new();
        task_group
            .spawn("task name", async { panic!("panic message") })
            .await;
        task_manager.await;
    }
    #[tokio::test]
    #[should_panic = r#"task "task name" panicked with "panic message 42""#]
    async fn it_propagates_formatted_panics() {
        let (task_group, task_manager) = super::infallible::TaskGroup::new();
        task_group
            .spawn("task name", async { panic!("panic message {}", 42) })
            .await;
        task_manager.await;
    }
    #[tokio::test]
    #[should_panic = r#"task "task name" panicked with [unprintable panic payload]"#]
    async fn it_propagates_unusual_panics() {
        let (task_group, task_manager) = super::infallible::TaskGroup::new();
        task_group
            .spawn("task name", async { std::panic::panic_any(42) })
            .await;
        task_manager.await;
    }
    #[tokio::test]
    async fn it_calls_drop() {
        use std::sync::{
            atomic::{AtomicBool, Ordering::SeqCst},
            Arc,
        };

        struct SetTrueOnDrop(Arc<AtomicBool>);
        impl Drop for SetTrueOnDrop {
            fn drop(&mut self) {
                self.0.store(true, SeqCst);
            }
        }

        let did_drop = Arc::new(AtomicBool::new(false));
        let did_drop_2 = Arc::clone(&did_drop);

        let (task_group, task_manager) = super::propagate_panics::TaskGroup::new();
        task_group
            .spawn("has drop", async {
                let _set_on_drop = SetTrueOnDrop(did_drop_2);
                futures::future::pending().await
            })
            .await;
        task_group.spawn("has error", async { Err(()) }).await;
        drop(task_group);
        assert_eq!(task_manager.await, Err(()));
        // TODO: Make task group more robust so that we don't need this delay for the test to pass
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert_eq!(did_drop.load(SeqCst), true);
    }
}
