use crate::interfaces::Spider;
use futures::stream::StreamExt;
use std::{ collections::HashSet, sync::{ atomic::{ AtomicUsize, Ordering }, Arc }, time::Duration };
use tokio::{ sync::{ mpsc, Barrier }, time::sleep };

pub struct Crawler {
    delay: Duration,
    concurrent_count: usize,
    processing_count: usize,
}

impl Crawler {
    pub fn new(delay: Duration, concurrent_count: usize, processing_count: usize) -> Self {
        Crawler {
            delay,
            concurrent_count,
            processing_count,
        }
    }

    pub async fn run<T: Send + 'static>(&self, spider: Arc<dyn Spider<Item = T>>) {
        let mut urls = HashSet::<String>::new();
        let concurrent_count = self.concurrent_count;
        let concurrent_queue_capacity = concurrent_count * 400;
        let processing_count = self.processing_count;
        let processing_queue_capacity = processing_count * 10;
        let active_spiders = Arc::new(AtomicUsize::new(0));

        let (urls_to_visit_tx, urls_to_visit_rx) = mpsc::channel(concurrent_queue_capacity);
        let (items_tx, items_rx) = mpsc::channel(processing_queue_capacity);
        let (new_urls_tx, mut new_urls_rx) = mpsc::channel(concurrent_queue_capacity);
        let barries = Arc::new(Barrier::new(3));

        for url in spider.start_urls() {
            urls.insert(url.clone());
            let _ = urls_to_visit_tx.send(url).await;
        }

        self.processors(processing_count, spider.clone(), items_rx, barries.clone());

        self.scrapes(concurrent_count, spider.clone(), urls_to_visit_rx, new_urls_tx.clone(),
            items_tx,
            active_spiders.clone(),
            self.delay,
            barries.clone());

        loop {
            if let Some((visited_url, new_urls)) = new_urls_rx.try_recv().ok() {
                urls.insert(visited_url);

                for url in new_urls {
                    if !urls.contains(&url) {
                        urls.insert(url.clone());
                        log::debug!("queueing : {}", url);
                        let _ = urls_to_visit_tx.send(url).await;
                    }
                }
            }

            if new_urls_tx.capacity() == concurrent_queue_capacity && urls_to_visit_tx.capacity() == concurrent_queue_capacity 
            && active_spiders.load(Ordering::SeqCst) == 0 {
                break;
            }

            sleep(Duration::from_millis(5)).await;
        }

        log::info!("crawler : control loop exited");

        drop(urls_to_visit_tx);

        barries.wait().await;
    }

    pub fn processors<T: Send + 'static>(&self, 
        concurrency: usize, 
        spider: Arc<dyn Spider<Item = T>>, 
        items: mpsc::Receiver<T>, 
        barrier: Arc<Barrier>) {
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(items)
                .for_each_concurrent(concurrency, |item| async {
                    let _ = spider.process(item).await;
                })
                .await;

            barrier.wait().await;
        });
    }

    pub fn scrapes<T: Send + 'static>(&self, 
        concurrency: usize,
        spider: Arc<dyn Spider<Item = T>>,
        urls_to_visit: mpsc::Receiver<String>,
        new_urls: mpsc::Sender<(String, Vec<String>)>,
        items_tx: mpsc::Sender<T>,
        active_spiders: Arc<AtomicUsize>,
        delay: Duration,
        barrier: Arc<Barrier>) {
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(urls_to_visit)
                .for_each_concurrent(concurrency, |queued_url| {
                    let queued_url = queued_url.clone();
                    async {
                        active_spiders.fetch_add(1, Ordering::SeqCst);
                        let mut urls = Vec::new();
                        let res = spider
                            .scrape(queued_url.clone())
                            .await
                            .map_err(|err| {
                                log::error!("{}", err);
                                err
                            })
                            .ok();

                        if let Some((items, new_urls)) = res {
                            for item in items {
                                let _ = items_tx.send(item).await;
                            }
                            urls = new_urls;
                        }

                        let _ = new_urls.send((queued_url, urls)).await;
                        sleep(delay).await;
                        active_spiders.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;

            drop(items_tx);
            barrier.wait().await;
        });
    }
}
