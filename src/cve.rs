use crate::error::Error;
use crate::interfaces::Spider;
use async_trait::async_trait;
use reqwest::Client;
use select::{ document::Document, predicate::{ Attr, Class, Name, Predicate } };
use std::time::Duration;

pub struct CveDetail {
    client: Client,
}

#[derive(Debug, Clone)]
pub struct Cve {
    name: String,
    url: String,
    cwe_id: Option<String>,
    cwe_uri: Option<String>,
    vulnerability_type: String,
    publish_date: String,
    update_date: String,
    score: f32,
    access: String,
    complexity: String,
    authentication: String,
    confidentiality: String,
    integrity: String,
    availability: String,
}

impl CveDetail {
    pub fn new() -> Self {
        let timeout = Duration::from_secs(6);
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("build http client expect!");
        CveDetail{ client }
    }
}

#[async_trait]
impl Spider for CveDetail {
    type Item = Cve;

    fn name(&self) -> String {
        String::from("cve-details")
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://www.cvedetails.com/vulnerability-list/vulnerabilities.htm".to_string()]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        log::info!("visiting : {}", url);
        let res = self.client.get(url).send().await?.text().await?;
        let mut items = Vec::new();
        let document = Document::from(res.as_str());

        let rows = document.select(Attr("id", "vulnslisttable").descendant(Class("srrowns")));
        for row in rows {
            let mut columns = row.select(Name("td"));
            let _ = columns.next();
            let cve_link = columns.next().unwrap().select(Name("a")).next().unwrap();
            let cve_name = cve_link.text().trim().to_string();
            let cve_url = self.url_join(cve_link.attr("href").unwrap());

            let cwe = columns
                .next()
                .unwrap()
                .select(Name("a"))
                .next()
                .map(|cwe_url| {
                    (
                        cwe_url.text().trim().to_string(),
                        self.url_join(cwe_url.attr("href").unwrap()),
                    )
                });

            let _ = columns.next();

            let vulnerability_type = columns.next().unwrap().text().trim().to_string();
            let publish_date = columns.next().unwrap().text().trim().to_string();
            let update_date = columns.next().unwrap().text().trim().to_string();

            let score: f32 = columns
                .next()
                .unwrap()
                .text()
                .trim()
                .to_string()
                .parse()
                .unwrap();

            let access = columns.next().unwrap().text().trim().to_string();
            let complexity = columns.next().unwrap().text().trim().to_string();
            let authentication = columns.next().unwrap().text().trim().to_string();
            let confidentiality = columns.next().unwrap().text().trim().to_string();
            let integrity = columns.next().unwrap().text().trim().to_string();
            let availability = columns.next().unwrap().text().trim().to_string();

            let cve = Cve {
                name: cve_name,
                url: cve_url,
                cwe_id: cwe.as_ref().map(|cwe| cwe.0.clone()),
                cwe_uri: cwe.as_ref().map(|cwe| cwe.1.clone()),
                vulnerability_type,
                publish_date,
                update_date,
                score,
                access,
                complexity,
                authentication,
                confidentiality,
                integrity,
                availability,
            };
            items.push(cve);
        }

        let next_page_links = document.select(Attr("id", "pagingb").descendant(Name("a")))
            .filter_map(|n| n.attr("href"))
            .map(|url| self.url_join(url))
            .collect::<Vec<String>>();

        Ok((items, next_page_links))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        println!("{:?}", item);
        Ok(())
    }
}

impl CveDetail {
    fn url_join(&self, url: &str) -> String {
        let url = url.trim();

        if url.starts_with("//www.cvedetails.com") {
            return format!("https:{}", url);
        } else if url.starts_with("/") {
            return format!("https://www.cvedetails.com{}", url);
        }

        return url.to_string();

    }
}
