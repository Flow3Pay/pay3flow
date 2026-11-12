//! Deterministic wallet avatars.
//!
//! The address is the only identity every offer row and deal row can reproduce
//! without a join, so the avatar is derived from it. The public key is stored
//! next to the avatar for future wallet-level identity checks, not mixed into
//! the picture: a seller address in the orderbook must always render the same
//! image as that wallet in the menu.
use super::{error::Result, repo};
use base64::{engine::general_purpose::STANDARD, Engine};
use deadpool_postgres::GenericClient;
use md5::{Digest, Md5};
use std::fmt::Write as _;

const GRID: usize = 5;
const SIZE: usize = 420;
const PADDING: usize = 42;

/// Stable SVG data URI for a wallet address.
pub(crate) fn identicon(address: &str) -> String {
    let svg = identicon_svg(address);
    format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))
}

/// SVG version of the deterministic identicon, shared with Telegram delivery.
pub(crate) fn identicon_svg(address: &str) -> String {
    let normalized = address.trim().to_ascii_lowercase();
    let digest = Md5::digest(normalized.as_bytes());
    let mut nibbles = [0u8; 32];
    for (byte_index, byte) in digest.iter().enumerate() {
        nibbles[byte_index * 2] = byte >> 4;
        nibbles[byte_index * 2 + 1] = byte & 0x0f;
    }

    let mut grid = [[false; GRID]; GRID];
    for index in 0..15 {
        let column = [2usize, 1, 0][index / GRID];
        let row = index % GRID;
        let filled = nibbles[index] % 2 == 0;
        grid[row][column] = filled;
        grid[row][GRID - 1 - column] = filled;
    }

    let hue = bits(&nibbles, 100, 12) as f64 / 4095.0 * 360.0;
    let saturation = 42.0 + bits(&nibbles, 112, 8) as f64 / 255.0 * 22.0;
    let lightness = 43.0 + bits(&nibbles, 120, 8) as f64 / 255.0 * 17.0;
    let (red, green, blue) = hsl(hue, saturation, lightness);

    let cell = (SIZE - PADDING * 2) as f64 / GRID as f64;
    let mut cells = String::new();
    for (row, line) in grid.iter().enumerate() {
        for (column, filled) in line.iter().enumerate() {
            if *filled {
                write!(
                    cells,
                    "<rect x=\"{}\" y=\"{}\" width=\"{cell}\" height=\"{cell}\"/>",
                    PADDING as f64 + column as f64 * cell,
                    PADDING as f64 + row as f64 * cell
                )
                .expect("writing to a String cannot fail");
            }
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SIZE}\" height=\"{SIZE}\" viewBox=\"0 0 {SIZE} {SIZE}\" shape-rendering=\"crispEdges\"><g fill=\"rgb({red},{green},{blue})\">{cells}</g></svg>"
    )
}

/// Persist the avatar for a proven wallet. Re-proving refreshes the public key
/// but never changes the image because the image is keyed by the address.
pub(crate) async fn record(
    client: &impl GenericClient,
    chain: &str,
    address: &str,
    public_key: Option<&str>,
) -> Result<()> {
    let address = address.trim().to_ascii_lowercase();
    let public_key = public_key.map(|key| {
        key.trim()
            .strip_prefix("0x")
            .unwrap_or(key.trim())
            .to_ascii_lowercase()
    });
    let avatar = identicon(&address);
    repo::avatar::record_avatar(client, chain, &address, &public_key, &avatar).await?;
    Ok(())
}

fn bits(nibbles: &[u8], start: usize, length: usize) -> u32 {
    let mut value = 0;
    for index in start..start + length {
        value = (value << 1) | u32::from((nibbles[index / 8] >> (7 - index % 8)) & 1);
    }
    value
}

fn hsl(hue: f64, saturation: f64, lightness: f64) -> (u8, u8, u8) {
    let saturation = saturation / 100.0;
    let lightness = lightness / 100.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (red, green, blue) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let offset = lightness - chroma / 2.0;
    let channel = |value: f64| ((value + offset) * 255.0).round().clamp(0.0, 255.0) as u8;
    (channel(red), channel(green), channel(blue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identicon_is_deterministic_and_embeddable() {
        let avatar =
            identicon("0:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert_eq!(
            avatar,
            identicon("0:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        );
        assert_ne!(
            avatar,
            identicon("0:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert!(avatar.starts_with("data:image/svg+xml;base64,"));
    }
}
