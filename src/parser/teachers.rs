use std::collections::HashMap;

use anyhow::{Context, anyhow};
use scraper::{ElementRef, Html, Selector};

use crate::structures::Teacher;

use super::urls;

pub async fn fetch_teachers(client: &reqwest::Client) -> anyhow::Result<Vec<Teacher>> {
    let search_key = get_search_key(client).await?;

    let mut form = HashMap::new();
    form.insert("search_key", search_key);
    form.insert("whatsearch", " ".to_string());

    let html = client
        .post(urls::HOST.to_string() + urls::SEARCH_TEACHERS)
        .form(&form)
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&html);
    let selector = Selector::parse(".pt-2.pb-4 a").unwrap();

    let teachers = doc
        .select(&selector)
        .map(process_teacher)
        .collect::<anyhow::Result<Vec<Teacher>>>()?;

    if teachers.is_empty() {
        Err(anyhow!("teachers list is empty"))
    } else {
        Ok(teachers)
    }
}

fn process_teacher(teacher: ElementRef) -> anyhow::Result<Teacher> {
    let href = teacher
        .value()
        .attr("href")
        .context("teacher has no href")?;

    let isu_id = href
        .split('/')
        .nth_back(1)
        .context("invalid href format")?
        .parse()
        .context("isu_id is not a number")?;

    let text = teacher.text().collect::<String>();
    let mut parts = text.splitn(2, '(');
    let name = parts.next().unwrap_or("").trim().to_string();

    let position = parts.next().unwrap_or("").trim_end_matches(')').to_string();

    Ok(Teacher {
        name,
        position,
        isu_id,
    })
}

async fn get_search_key(client: &reqwest::Client) -> anyhow::Result<String> {
    let html = client
        .get(urls::HOST.to_string() + urls::SEARCH_TEACHERS)
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&html);
    let selector = Selector::parse(r#"input[name="search_key"]"#).unwrap();

    doc.select(&selector)
        .find_map(|el| el.value().attr("value"))
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("search_key not found"))
}
