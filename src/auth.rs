use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json;
use std::error::Error;

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: String,
}

pub async fn verify_clerk_token(token: &str) -> Result<Claims, Box<dyn Error>> {
    let header = decode_header(token)?;
    let kid = header.kid.ok_or("No 'kid' in token header")?;

    // Use the correct JWKS URL
    let jwks_url = "https://massive-filly-39.clerk.accounts.dev/.well-known/jwks.json";

    let jwks: serde_json::Value = reqwest::get(jwks_url).await?.json().await?;

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
