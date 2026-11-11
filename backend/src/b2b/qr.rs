//! Minimal QR Code encoder for the hosted payment page.
//!
//! Only byte mode and error-correction level M are needed: the page encodes a
//! short payment URI or deposit address. The encoder is deliberately
//! dependency-free so the backend does not need a native QR library at
//! runtime.

const TOTAL_CODEWORDS: [usize; 11] = [0, 26, 44, 70, 100, 134, 172, 196, 242, 292, 346];
const EC_CODEWORDS_PER_BLOCK: [usize; 11] = [0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26];
const EC_BLOCKS: [usize; 11] = [0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5];
const EC_LEVEL_BITS: u32 = 0b00; // level M

/// Encode `data` (byte mode, ECC level M) and return the module matrix.
pub fn matrix(data: &str) -> Option<Vec<Vec<bool>>> {
    let bytes = data.as_bytes();
    let version = (1..=10).find(|version| {
        let data_codewords =
            TOTAL_CODEWORDS[*version] - EC_CODEWORDS_PER_BLOCK[*version] * EC_BLOCKS[*version];
        let count_bits = if *version <= 9 { 8 } else { 16 };
        4 + count_bits + bytes.len() * 8 <= data_codewords * 8
    })?;
    let codewords = codewords(version, bytes);
    let size = 17 + 4 * version;
    let mut canvas = Canvas::new(size, version);
    canvas.draw_data(&codewords);
    let mut best: Option<(i64, Vec<Vec<bool>>)> = None;
    for mask in 0..8u8 {
        let mut candidate = canvas.clone();
        candidate.apply_mask(mask);
        candidate.draw_format(mask);
        if candidate.version >= 7 {
            candidate.draw_version();
        }
        let penalty = candidate.penalty();
        if best.as_ref().is_none_or(|(score, _)| penalty < *score) {
            best = Some((penalty, candidate.modules));
        }
    }
    best.map(|(_, modules)| modules)
}

/// Render a QR matrix as a self-contained SVG string.
pub fn svg(data: &str) -> Option<String> {
    let modules = matrix(data)?;
    let size = modules.len();
    let quiet = 4;
    let full = size + quiet * 2;
    let mut path = String::new();
    for (row, modules) in modules.iter().enumerate() {
        let mut col = 0;
        while col < size {
            if !modules[col] {
                col += 1;
                continue;
            }
            let start = col;
            while col < size && modules[col] {
                col += 1;
            }
            path.push_str(&format!(
                "M{},{}h{}v1h-{}z",
                start + quiet,
                row + quiet,
                col - start,
                col - start
            ));
        }
    }
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {full} {full}" width="220" height="220" role="img" aria-label="QR code"><rect width="{full}" height="{full}" fill="#fff"/><path d="{path}" fill="#000"/></svg>"##
    ))
}

fn codewords(version: usize, bytes: &[u8]) -> Vec<u8> {
    let data_codewords =
        TOTAL_CODEWORDS[version] - EC_CODEWORDS_PER_BLOCK[version] * EC_BLOCKS[version];
    let capacity_bits = data_codewords * 8;
    let count_bits = if version <= 9 { 8 } else { 16 };
    let mut bits = Vec::with_capacity(capacity_bits);
    push_bits(&mut bits, 0b0100, 4);
    push_bits(&mut bits, bytes.len() as u32, count_bits);
    for byte in bytes {
        push_bits(&mut bits, u32::from(*byte), 8);
    }
    let terminator = (capacity_bits - bits.len()).min(4);
    push_bits(&mut bits, 0, terminator);
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    let mut data = Vec::with_capacity(data_codewords);
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for bit in chunk {
            byte = (byte << 1) | u8::from(*bit);
        }
        data.push(byte);
    }
    for pad in [0xEC, 0x11].into_iter().cycle() {
        if data.len() >= data_codewords {
            break;
        }
        data.push(pad);
    }

    let block_count = EC_BLOCKS[version];
    let ec_len = EC_CODEWORDS_PER_BLOCK[version];
    let short_block_len = TOTAL_CODEWORDS[version] / block_count;
    let short_blocks = block_count - TOTAL_CODEWORDS[version] % block_count;
    let mut blocks = Vec::with_capacity(block_count);
    let mut offset = 0;
    for index in 0..block_count {
        let data_len = short_block_len - ec_len + usize::from(index >= short_blocks);
        let block_data = data[offset..offset + data_len].to_vec();
        offset += data_len;
        let ecc = reed_solomon(&block_data, ec_len);
        blocks.push((block_data, ecc));
    }
    let mut result = Vec::with_capacity(TOTAL_CODEWORDS[version]);
    let max_data = blocks.iter().map(|(data, _)| data.len()).max().unwrap_or(0);
    for index in 0..max_data {
        for (data, _) in &blocks {
            if let Some(byte) = data.get(index) {
                result.push(*byte);
            }
        }
    }
    for index in 0..ec_len {
        for (_, ecc) in &blocks {
            result.push(ecc[index]);
        }
    }
    result
}

fn push_bits(bits: &mut Vec<bool>, value: u32, count: usize) {
    for shift in (0..count).rev() {
        bits.push((value >> shift) & 1 == 1);
    }
}

fn reed_solomon(data: &[u8], ec_len: usize) -> Vec<u8> {
    let mut generator = vec![1u8];
    let mut root = 1u8;
    for _ in 0..ec_len {
        let mut next = vec![0u8; generator.len() + 1];
        for (index, coefficient) in generator.iter().enumerate() {
            next[index] ^= gf_mul(*coefficient, root);
            next[index + 1] ^= *coefficient;
        }
        generator = next;
        root = gf_mul(root, 2);
    }
    let mut remainder = vec![0u8; ec_len];
    for byte in data {
        let factor = byte ^ remainder[0];
        remainder.remove(0);
        remainder.push(0);
        for (index, coefficient) in generator.iter().skip(1).enumerate() {
            remainder[index] ^= gf_mul(*coefficient, factor);
        }
    }
    remainder
}

fn gf_mul(mut left: u8, mut right: u8) -> u8 {
    let mut result = 0u8;
    while right != 0 {
        if right & 1 != 0 {
            result ^= left;
        }
        let high = left & 0x80 != 0;
        left <<= 1;
        if high {
            left ^= 0x1D;
        }
        right >>= 1;
    }
    result
}

#[derive(Clone)]
struct Canvas {
    modules: Vec<Vec<bool>>,
    function: Vec<Vec<bool>>,
    version: usize,
}

impl Canvas {
    fn new(size: usize, version: usize) -> Self {
        let mut canvas = Self {
            modules: vec![vec![false; size]; size],
            function: vec![vec![false; size]; size],
            version,
        };
        canvas.draw_function_patterns();
        canvas
    }

    fn set_function(&mut self, row: usize, col: usize, dark: bool) {
        self.modules[row][col] = dark;
        self.function[row][col] = true;
    }

    fn draw_function_patterns(&mut self) {
        let size = self.modules.len();
        for (row, col) in [(0usize, 0usize), (0, size - 7), (size - 7, 0)] {
            for dr in 0..9usize {
                for dc in 0..9usize {
                    let r = row as isize + dr as isize - 1;
                    let c = col as isize + dc as isize - 1;
                    if r < 0 || c < 0 || r >= size as isize || c >= size as isize {
                        continue;
                    }
                    let inside = (1..=7).contains(&dr) && (1..=7).contains(&dc);
                    let ring = matches!(dr, 1 | 7) || matches!(dc, 1 | 7);
                    let center = (3..=5).contains(&dr) && (3..=5).contains(&dc);
                    self.set_function(r as usize, c as usize, inside && (ring || center));
                }
            }
        }
        for index in 8..size - 8 {
            let dark = index % 2 == 0;
            self.set_function(6, index, dark);
            self.set_function(index, 6, dark);
        }
        for row in &self.alignment_positions() {
            for col in &self.alignment_positions() {
                if *row == 6 && *col == 6 {
                    continue;
                }
                let near_top_left = *row < 8 && *col < 8;
                let near_top_right = *row < 8 && *col > size - 9;
                let near_bottom_left = *row > size - 9 && *col < 8;
                if near_top_left || near_top_right || near_bottom_left {
                    continue;
                }
                for dr in 0..5usize {
                    for dc in 0..5usize {
                        let dark =
                            matches!(dr, 0 | 4) || matches!(dc, 0 | 4) || (dr == 2 && dc == 2);
                        self.set_function(row + dr - 2, col + dc - 2, dark);
                    }
                }
            }
        }
        for index in 0..9 {
            if index != 6 {
                self.set_function(8, index, false);
                self.set_function(index, 8, false);
            }
        }
        for index in 0..8 {
            self.set_function(8, size - 1 - index, false);
            self.set_function(size - 1 - index, 8, false);
        }
        self.set_function(size - 8, 8, true);
        if self.version >= 7 {
            for index in 0..6 {
                for offset in 0..3 {
                    self.set_function(index, size - 11 + offset, false);
                    self.set_function(size - 11 + offset, index, false);
                }
            }
        }
    }

    fn alignment_positions(&self) -> Vec<usize> {
        match self.version {
            1 => Vec::new(),
            2 => vec![6, 18],
            3 => vec![6, 22],
            4 => vec![6, 26],
            5 => vec![6, 30],
            6 => vec![6, 34],
            7 => vec![6, 22, 38],
            8 => vec![6, 24, 42],
            9 => vec![6, 26, 46],
            10 => vec![6, 28, 50],
            _ => Vec::new(),
        }
    }

    fn draw_data(&mut self, codewords: &[u8]) {
        let size = self.modules.len();
        let mut bits = Vec::with_capacity(codewords.len() * 8);
        for byte in codewords {
            for shift in (0..8).rev() {
                bits.push((byte >> shift) & 1 == 1);
            }
        }
        let mut index = 0usize;
        let mut right = size as isize - 1;
        let mut upward = true;
        while right > 0 {
            if right == 6 {
                right -= 1;
            }
            for step in 0..size {
                let row = if upward { size - 1 - step } else { step };
                for column in [right, right - 1] {
                    let column = column as usize;
                    if self.function[row][column] {
                        continue;
                    }
                    self.modules[row][column] = bits.get(index).copied().unwrap_or(false);
                    index += 1;
                }
            }
            upward = !upward;
            right -= 2;
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        let size = self.modules.len();
        for row in 0..size {
            for col in 0..size {
                if self.function[row][col] {
                    continue;
                }
                let invert = match mask {
                    0 => (row + col) % 2 == 0,
                    1 => row % 2 == 0,
                    2 => col % 3 == 0,
                    3 => (row + col) % 3 == 0,
                    4 => (row / 2 + col / 3) % 2 == 0,
                    5 => (row * col) % 2 + (row * col) % 3 == 0,
                    6 => ((row * col) % 2 + (row * col) % 3) % 2 == 0,
                    7 => ((row + col) % 2 + (row * col) % 3) % 2 == 0,
                    _ => false,
                };
                if invert {
                    self.modules[row][col] = !self.modules[row][col];
                }
            }
        }
    }

    fn draw_format(&mut self, mask: u8) {
        let size = self.modules.len();
        let data = (EC_LEVEL_BITS << 3) | u32::from(mask);
        let mut remainder = data;
        for _ in 0..10 {
            remainder = (remainder << 1) ^ ((remainder >> 9) * 0x537);
        }
        let bits = ((data << 10) | remainder) ^ 0x5412;
        for index in 0..15usize {
            let dark = (bits >> index) & 1 == 1;
            let (row, col) = match index {
                0 => (8, 0),
                1 => (8, 1),
                2 => (8, 2),
                3 => (8, 3),
                4 => (8, 4),
                5 => (8, 5),
                6 => (8, 7),
                7 => (8, 8),
                8 => (7, 8),
                9 => (5, 8),
                10 => (4, 8),
                11 => (3, 8),
                12 => (2, 8),
                13 => (1, 8),
                14 => (0, 8),
                _ => unreachable!(),
            };
            self.modules[row][col] = dark;
        }
        // Seven bits next to the bottom-left finder; the eighth position is
        // the fixed dark module.
        for index in 0..7usize {
            self.modules[size - 1 - index][8] = (bits >> index) & 1 == 1;
        }
        for index in 8..15usize {
            self.modules[8][size - 15 + index] = (bits >> index) & 1 == 1;
        }
    }

    fn draw_version(&mut self) {
        let size = self.modules.len();
        let version = self.version as u32;
        let mut remainder = version;
        for _ in 0..12 {
            remainder = (remainder << 1) ^ ((remainder >> 11) * 0x1F25);
        }
        let bits = (version << 12) | remainder;
        for index in 0..18usize {
            let dark = (bits >> index) & 1 == 1;
            let row = index / 3;
            let col = size - 11 + index % 3;
            self.modules[row][col] = dark;
            self.modules[col][row] = dark;
        }
    }

    fn penalty(&self) -> i64 {
        let size = self.modules.len();
        let mut score = 0i64;
        for row in 0..size {
            score += line_penalty(&self.modules[row]);
            let column = (0..size).map(|r| self.modules[r][row]).collect::<Vec<_>>();
            score += line_penalty(&column);
        }
        for row in 0..size - 1 {
            for col in 0..size - 1 {
                let color = self.modules[row][col];
                if self.modules[row][col + 1] == color
                    && self.modules[row + 1][col] == color
                    && self.modules[row + 1][col + 1] == color
                {
                    score += 3;
                }
            }
        }
        let dark = self
            .modules
            .iter()
            .flatten()
            .filter(|module| **module)
            .count();
        let percent = dark * 100 / (size * size);
        let deviation = (percent as i64 - 50).abs();
        score += (deviation / 5) * 10;
        score
    }
}

fn line_penalty(line: &[bool]) -> i64 {
    let mut score = 0i64;
    let mut run = 1;
    for index in 1..line.len() {
        if line[index] == line[index - 1] {
            run += 1;
        } else {
            if run >= 5 {
                score += 3 + (run - 5) as i64;
            }
            run = 1;
        }
    }
    if run >= 5 {
        score += 3 + (run - 5) as i64;
    }
    for window in line.windows(11) {
        let plain = [
            true, false, true, true, true, false, true, false, false, false, false,
        ];
        let reversed = [
            false, false, false, false, true, false, true, true, true, false, true,
        ];
        if window == plain.as_slice() || window == reversed.as_slice() {
            score += 40;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_svg_for_short_payloads() {
        let svg = svg("0x8ba1f109551bD432803012645Ac136ddd64DBA72").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("<path"));
    }

    #[test]
    fn rejects_payloads_larger_than_version_10() {
        assert!(matrix(&"a".repeat(300)).is_none());
    }

    #[test]
    fn same_payload_encodes_deterministically() {
        assert_eq!(matrix("hello").unwrap(), matrix("hello").unwrap());
    }
}
