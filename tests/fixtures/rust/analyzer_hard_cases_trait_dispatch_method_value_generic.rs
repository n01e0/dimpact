pub trait Worker<T> {
    fn handle(&self, v: T) -> T;
}

pub struct Pipeline<W> {
    pub worker: W,
}

impl<W, T> Pipeline<W>
where
    W: Worker<T>,
    T: Clone,
{
    pub fn run(&self, input: T) -> T {
        let method = Worker::handle;
        let first = method(&self.worker, input.clone());
        self.worker.handle(first)
    }
}
