use reqwest::Client;
use serde::Deserialize;
use tokio;
use postgres::{Client as PgClient, NoTls};
use std::env;
use std::fs::File;
use std::io::Write;

#[derive(Deserialize)]
struct PublicationIdResponse {
    publication_id: String,
}

#[derive(Deserialize)]
struct ArticlesResponse {
    publication_articles: Vec<String>,
}

#[derive(Deserialize)]
struct ArticleContent {
    id: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    audio_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    
    let medium_api_key = env::var("MEDIUM_API_KEY").expect("MEDIUM_API_KEY must be set");
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let pg_conn_str = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let publication_slug = "your-publication-slug"; // Replace with your publication slug

    let client = Client::new();
    let publication_id = fetch_publication_id(&client, &medium_api_key, publication_slug).await?;
    let article_ids = fetch_publication_articles(&client, &medium_api_key, &publication_id).await?;
    let article_content = fetch_article_content(&client, &medium_api_key, &article_ids[0]).await?;
    save_to_postgres(&pg_conn_str, &article_content)?;
    let audio_url = convert_text_to_audio(&client, &openai_api_key, &article_content.content).await?;
    download_audio(&audio_url).await?;

    Ok(())
}

async fn fetch_publication_id(client: &Client, api_key: &str, slug: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://medium2.p.rapidapi.com/publication/id_for/{}", slug);
    let res = client.get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    let text = res.text().await?;
    println!("Publication ID response: {}", text);

    let response: PublicationIdResponse = serde_json::from_str(&text)?;
    Ok(response.publication_id)
}

async fn fetch_publication_articles(client: &Client, api_key: &str, publication_id: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let url = format!("https://medium2.p.rapidapi.com/publication/{}/articles", publication_id);
    let res = client.get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    let text = res.text().await?;
    println!("Publication articles response: {}", text);

    let response: ArticlesResponse = serde_json::from_str(&text)?;
    Ok(response.publication_articles)
}

async fn fetch_article_content(client: &Client, api_key: &str, article_id: &str) -> Result<ArticleContent, Box<dyn std::error::Error>> {
    let url = format!("https://medium2.p.rapidapi.com/article/{}/content", article_id);
    let res = client.get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    let text = res.text().await?;
    println!("Article content response: {}", text);

    let response: ArticleContent = serde_json::from_str(&text)?;
    Ok(response)
}

fn save_to_postgres(conn_str: &str, article: &ArticleContent) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PgClient::connect(conn_str, NoTls)?;
    client.execute(
        "CREATE TABLE IF NOT EXISTS articles (id VARCHAR PRIMARY KEY, content TEXT)",
        &[],
    )?;
    client.execute(
        "INSERT INTO articles (id, content) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
        &[&article.id, &article.content],
    )?;
    Ok(())
}

async fn convert_text_to_audio(client: &Client, api_key: &str, text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = "https://api.openai.com/v1/audio/create";
    let res = client.post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({ "input": text }))
        .send()
        .await?
        .json::<OpenAIResponse>()
        .await?;

    Ok(res.audio_url)
}

async fn download_audio(audio_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut res = reqwest::get(audio_url).await?;
    let mut file = File::create("audiobook.mp3")?;
    while let Some(chunk) = res.chunk().await? {
        file.write_all(&chunk)?;
    }

    Ok(())
}
