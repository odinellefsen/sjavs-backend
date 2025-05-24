use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use once_cell::sync::OnceCell;
use serde::Deserialize;
use serde_json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: String,
}

// Cache structure to hold JWKS data with TTL
#[derive(Clone)]
struct JwksCache {
    data: serde_json::Value,
    fetched_at: Instant,
    ttl: Duration,
}

impl JwksCache {
    fn new(data: serde_json::Value, ttl: Duration) -> Self {
        Self {
            data,
            fetched_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

// Global cache instance
static JWKS_CACHE: OnceCell<Arc<RwLock<Option<JwksCache>>>> = OnceCell::new();

// Initialize the cache
fn get_cache() -> &'static Arc<RwLock<Option<JwksCache>>> {
    JWKS_CACHE.get_or_init(|| Arc::new(RwLock::new(None)))
}

// Fetch JWKS from Clerk with caching
async fn get_jwks() -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync + 'static>>
{
    let cache = get_cache();
    let ttl = Duration::from_secs(3600); // 1 hour TTL

    // First, try to read from cache
    {
        let cache_read = cache.read().await;
        if let Some(cached_jwks) = cache_read.as_ref() {
            if !cached_jwks.is_expired() {
                // Cache hit - return cached data
                return Ok(cached_jwks.data.clone());
            }
        }
    }

    // Cache miss or expired - need to fetch new data
    // Upgrade to write lock
    let mut cache_write = cache.write().await;

    // Double-check pattern: another thread might have updated the cache
    // while we were waiting for the write lock
    if let Some(cached_jwks) = cache_write.as_ref() {
        if !cached_jwks.is_expired() {
            return Ok(cached_jwks.data.clone());
        }
    }

    // Actually fetch the JWKS data
    let jwks_url = "https://massive-filly-39.clerk.accounts.dev/.well-known/jwks.json";

    println!("Fetching JWKS from Clerk (cache miss or expired)");

    match reqwest::get(jwks_url).await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(jwks) => {
                    // Update cache with new data
                    *cache_write = Some(JwksCache::new(jwks.clone(), ttl));
                    println!("JWKS cache updated successfully");
                    Ok(jwks)
                }
                Err(e) => {
                    // If we have expired cache data, use it as fallback
                    if let Some(cached_jwks) = cache_write.as_ref() {
                        println!("JWKS fetch failed, using expired cache as fallback: {}", e);
                        Ok(cached_jwks.data.clone())
                    } else {
                        Err(
                            format!("Failed to parse JWKS JSON and no cache available: {}", e)
                                .into(),
                        )
                    }
                }
            }
        }
        Err(e) => {
            // If we have expired cache data, use it as fallback
            if let Some(cached_jwks) = cache_write.as_ref() {
                println!("JWKS fetch failed, using expired cache as fallback: {}", e);
                Ok(cached_jwks.data.clone())
            } else {
                Err(format!("Failed to fetch JWKS and no cache available: {}", e).into())
            }
        }
    }
}

pub async fn verify_clerk_token(
    token: &str,
) -> Result<Claims, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let header = decode_header(token)?;
    let kid = header.kid.ok_or("No 'kid' in token header")?;

    // Get JWKS data (from cache or fresh fetch)
    let jwks = get_jwks().await?;

    // Rest of the verification logic
    let matching_key = jwks["keys"]
        .as_array()
        .ok_or("No keys found")?
        .iter()
        .find(|k| k["kid"].as_str() == Some(&kid))
        .ok_or("No matching key found")?;

    let n = matching_key["n"].as_str().ok_or("No 'n' in JWK")?;
    let e = matching_key["e"].as_str().ok_or("No 'e' in JWK")?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://massive-filly-39.clerk.accounts.dev"]);

    let key = DecodingKey::from_rsa_components(n, e)?;
    let token_data = decode::<Claims>(token, &key, &validation)?;

    Ok(token_data.claims)
}
