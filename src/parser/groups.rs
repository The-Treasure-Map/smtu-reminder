use anyhow::Context;
use scraper::{ElementRef, Html, Selector};

use crate::structures::Group;

use super::urls;

pub async fn fetch_groups(client: &reqwest::Client) -> anyhow::Result<Vec<Group>> {
    let html = client
        .get(urls::HOST.to_string() + urls::GROUPS_PAGE)
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&html);
    let selector = Selector::parse(".gr").unwrap();

    doc.select(&selector).map(process_group).collect()
}

fn process_group(group: ElementRef) -> anyhow::Result<Group> {
    let href = group
        .select(&Selector::parse("a").unwrap())
        .next()
        .context("group have no link")?
        .value()
        .attr("href")
        .context("group has no href")?;

    let id = href
        .split('/')
        .nth_back(1)
        .context("invalid href format")?
        .trim()
        .parse()
        .context("id is not a number")?;

    let number = group
        .text()
        .collect::<String>()
        .trim()
        .parse()
        .context("group number is not a number")?;

    Ok(Group { number, id })
}
