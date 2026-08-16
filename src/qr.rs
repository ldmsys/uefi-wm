/*
 * QR encoding logic is adapted from Project Nayuki's QR Code generator.
 * Copyright (c) Project Nayuki. (MIT License)
 * https://www.nayuki.io/page/qr-code-generator-library
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

//! Dependency-free QR Code encoding used by the window-manager widget.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

const MIN_VERSION: u8 = 1;
const MAX_VERSION: u8 = 10;
const ECC_PER_BLOCK: [usize; 10] = [7, 10, 15, 20, 26, 18, 20, 24, 30, 18];
const NUM_BLOCKS: [usize; 10] = [1, 1, 1, 1, 1, 2, 2, 2, 2, 4];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncodeError {
    DataTooLong,
}

pub(crate) struct QrSymbol {
    size: usize,
    modules: Vec<bool>,
}

impl QrSymbol {
    pub(crate) fn encode(data: &[u8]) -> Result<Self, EncodeError> {
        let version = (MIN_VERSION..=MAX_VERSION)
            .find(|&version| required_bits(data.len(), version) <= data_capacity_bits(version))
            .ok_or(EncodeError::DataTooLong)?;

        let data_codewords = make_data_codewords(data, version);
        let all_codewords = add_error_correction(&data_codewords, version);
        let size = version as usize * 4 + 17;
        let mut qr = Self {
            size,
            modules: vec![false; size * size],
        };
        let mut function = vec![false; size * size];
        qr.draw_function_patterns(version, &mut function);
        qr.draw_codewords(&all_codewords, &function);

        let mut best_mask = 0;
        let mut best_penalty = u32::MAX;
        for mask in 0..8 {
            qr.apply_mask(mask, &function);
            qr.draw_format_bits(mask);
            let penalty = qr.penalty_score();
            if penalty < best_penalty {
                best_penalty = penalty;
                best_mask = mask;
            }
            qr.apply_mask(mask, &function);
        }
        qr.apply_mask(best_mask, &function);
        qr.draw_format_bits(best_mask);
        Ok(qr)
    }

    pub(crate) const fn size(&self) -> usize {
        self.size
    }

    pub(crate) fn module(&self, x: usize, y: usize) -> bool {
        self.modules[y * self.size + x]
    }

    fn set(&mut self, x: usize, y: usize, dark: bool) {
        self.modules[y * self.size + x] = dark;
    }

    fn set_function(&mut self, function: &mut [bool], x: usize, y: usize, dark: bool) {
        self.set(x, y, dark);
        function[y * self.size + x] = true;
    }

    fn draw_function_patterns(&mut self, version: u8, function: &mut [bool]) {
        for i in 0..self.size {
            self.set_function(function, 6, i, i % 2 == 0);
            self.set_function(function, i, 6, i % 2 == 0);
        }
        self.draw_finder(function, 3, 3);
        self.draw_finder(function, self.size - 4, 3);
        self.draw_finder(function, 3, self.size - 4);

        let align = alignment_positions(version);
        for (i, &x) in align.iter().enumerate() {
            for (j, &y) in align.iter().enumerate() {
                let last = align.len() - 1;
                if (i == 0 && (j == 0 || j == last)) || (i == last && j == 0) {
                    continue;
                }
                self.draw_alignment(function, x, y);
            }
        }
        self.draw_format_function_modules(function);
        if version >= 7 {
            self.draw_version_bits(version, function);
        }
    }

    fn draw_finder(&mut self, function: &mut [bool], cx: usize, cy: usize) {
        for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                let x = cx as i32 + dx;
                let y = cy as i32 + dy;
                if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
                    continue;
                }
                let distance = dx.abs().max(dy.abs());
                self.set_function(
                    function,
                    x as usize,
                    y as usize,
                    distance != 2 && distance != 4,
                );
            }
        }
    }

    fn draw_alignment(&mut self, function: &mut [bool], cx: usize, cy: usize) {
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let distance = dx.abs().max(dy.abs());
                self.set_function(
                    function,
                    (cx as i32 + dx) as usize,
                    (cy as i32 + dy) as usize,
                    distance != 1,
                );
            }
        }
    }

    fn draw_format_function_modules(&mut self, function: &mut [bool]) {
        for i in 0..=5 {
            self.set_function(function, 8, i, false);
        }
        self.set_function(function, 8, 7, false);
        self.set_function(function, 8, 8, false);
        self.set_function(function, 7, 8, false);
        for i in 9..15 {
            self.set_function(function, 14 - i, 8, false);
        }
        for i in 0..8 {
            self.set_function(function, self.size - 1 - i, 8, false);
        }
        for i in 8..15 {
            self.set_function(function, 8, self.size - 15 + i, false);
        }
        self.set_function(function, 8, self.size - 8, true);
    }

    fn draw_format_bits(&mut self, mask: u8) {
        let data = (1u32 << 3) | mask as u32; // Error correction level Low.
        let mut rem = data;
        for _ in 0..10 {
            rem = (rem << 1) ^ ((rem >> 9) * 0x537);
        }
        let bits = ((data << 10) | rem) ^ 0x5412;
        let bit = |i: usize| ((bits >> i) & 1) != 0;

        for i in 0..=5 {
            self.set(8, i, bit(i));
        }
        self.set(8, 7, bit(6));
        self.set(8, 8, bit(7));
        self.set(7, 8, bit(8));
        for i in 9..15 {
            self.set(14 - i, 8, bit(i));
        }
        for i in 0..8 {
            self.set(self.size - 1 - i, 8, bit(i));
        }
        for i in 8..15 {
            self.set(8, self.size - 15 + i, bit(i));
        }
        self.set(8, self.size - 8, true);
    }

    fn draw_version_bits(&mut self, version: u8, function: &mut [bool]) {
        let mut rem = version as u32;
        for _ in 0..12 {
            rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
        }
        let bits = ((version as u32) << 12) | rem;
        for i in 0..18 {
            let dark = ((bits >> i) & 1) != 0;
            let a = self.size - 11 + i % 3;
            let b = i / 3;
            self.set_function(function, a, b, dark);
            self.set_function(function, b, a, dark);
        }
    }

    fn draw_codewords(&mut self, data: &[u8], function: &[bool]) {
        let mut bit_index = 0usize;
        let mut right = self.size as i32 - 1;
        while right >= 1 {
            if right == 6 {
                right = 5;
            }
            for vertical in 0..self.size {
                let upward = ((right + 1) & 2) == 0;
                let y = if upward {
                    self.size - 1 - vertical
                } else {
                    vertical
                };
                for column in 0..2 {
                    let x = (right - column) as usize;
                    if !function[y * self.size + x] && bit_index < data.len() * 8 {
                        self.set(
                            x,
                            y,
                            ((data[bit_index >> 3] >> (7 - (bit_index & 7))) & 1) != 0,
                        );
                        bit_index += 1;
                    }
                }
            }
            right -= 2;
        }
    }

    fn apply_mask(&mut self, mask: u8, function: &[bool]) {
        for y in 0..self.size {
            for x in 0..self.size {
                let invert = match mask {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => (x / 3 + y / 2) % 2 == 0,
                    5 => x * y % 2 + x * y % 3 == 0,
                    6 => (x * y % 2 + x * y % 3) % 2 == 0,
                    _ => ((x + y) % 2 + x * y % 3) % 2 == 0,
                };
                if invert && !function[y * self.size + x] {
                    let index = y * self.size + x;
                    self.modules[index] = !self.modules[index];
                }
            }
        }
    }

    fn penalty_score(&self) -> u32 {
        let mut result = 0u32;
        for y in 0..self.size {
            result += line_penalty((0..self.size).map(|x| self.module(x, y)));
        }
        for x in 0..self.size {
            result += line_penalty((0..self.size).map(|y| self.module(x, y)));
        }
        for y in 0..self.size - 1 {
            for x in 0..self.size - 1 {
                let color = self.module(x, y);
                if self.module(x + 1, y) == color
                    && self.module(x, y + 1) == color
                    && self.module(x + 1, y + 1) == color
                {
                    result += 3;
                }
            }
        }
        let dark = self.modules.iter().filter(|&&module| module).count();
        let total = self.modules.len();
        result + ((dark * 20).abs_diff(total * 10) / total * 10) as u32
    }
}

fn line_penalty(line: impl Iterator<Item = bool>) -> u32 {
    let modules: Vec<bool> = line.collect();
    let mut result = 0;
    let mut run = 1usize;
    for i in 1..modules.len() {
        if modules[i] == modules[i - 1] {
            run += 1;
            if run == 5 {
                result += 3;
            } else if run > 5 {
                result += 1;
            }
        } else {
            run = 1;
        }
    }
    for window in modules.windows(11) {
        if window
            == [
                true, false, true, true, true, false, true, false, false, false, false,
            ]
            || window
                == [
                    false, false, false, false, true, false, true, true, true, false, true,
                ]
        {
            result += 40;
        }
    }
    result
}

fn required_bits(data_len: usize, version: u8) -> usize {
    4 + if version <= 9 { 8 } else { 16 } + data_len.saturating_mul(8)
}

fn data_capacity_bits(version: u8) -> usize {
    let index = version as usize - 1;
    (raw_data_modules(version) / 8 - ECC_PER_BLOCK[index] * NUM_BLOCKS[index]) * 8
}

fn raw_data_modules(version: u8) -> usize {
    let version = version as usize;
    let mut result = (16 * version + 128) * version + 64;
    if version >= 2 {
        let align = version / 7 + 2;
        result -= (25 * align - 10) * align - 55;
        if version >= 7 {
            result -= 36;
        }
    }
    result
}

fn alignment_positions(version: u8) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let count = version as usize / 7 + 2;
    let step = ((version as usize * 4 + count * 2 + 1) / (count * 2 - 2)) * 2;
    let size = version as usize * 4 + 17;
    let mut result = vec![6];
    for i in (0..count - 1).rev() {
        result.push(size - 7 - i * step);
    }
    result
}

fn make_data_codewords(data: &[u8], version: u8) -> Vec<u8> {
    let capacity = data_capacity_bits(version);
    let mut bits = Vec::with_capacity(capacity);
    append_bits(&mut bits, 0b0100, 4);
    append_bits(
        &mut bits,
        data.len() as u32,
        if version <= 9 { 8 } else { 16 },
    );
    for &byte in data {
        append_bits(&mut bits, byte as u32, 8);
    }
    let terminator = (capacity - bits.len()).min(4);
    bits.resize(bits.len() + terminator, false);
    while bits.len() % 8 != 0 {
        bits.push(false);
    }

    let mut result = Vec::with_capacity(capacity / 8);
    for chunk in bits.chunks_exact(8) {
        result.push(
            chunk
                .iter()
                .fold(0, |value, &bit| (value << 1) | u8::from(bit)),
        );
    }
    let mut pad = 0xEC;
    while result.len() < capacity / 8 {
        result.push(pad);
        pad ^= 0xEC ^ 0x11;
    }
    result
}

fn append_bits(bits: &mut Vec<bool>, value: u32, count: usize) {
    for i in (0..count).rev() {
        bits.push(((value >> i) & 1) != 0);
    }
}

fn add_error_correction(data: &[u8], version: u8) -> Vec<u8> {
    let index = version as usize - 1;
    let block_count = NUM_BLOCKS[index];
    let ecc_len = ECC_PER_BLOCK[index];
    let raw_codewords = raw_data_modules(version) / 8;
    let short_block_len = raw_codewords / block_count;
    let short_block_count = block_count - raw_codewords % block_count;
    let short_data_len = short_block_len - ecc_len;
    let divisor = reed_solomon_divisor(ecc_len);
    let mut blocks: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(block_count);
    let mut offset = 0;
    for block in 0..block_count {
        let length = short_data_len + usize::from(block >= short_block_count);
        let block_data = data[offset..offset + length].to_vec();
        let ecc = reed_solomon_remainder(&block_data, &divisor);
        blocks.push((block_data, ecc));
        offset += length;
    }

    let mut result = Vec::with_capacity(raw_codewords);
    for column in 0..=short_data_len {
        for (block_data, _) in &blocks {
            if let Some(&value) = block_data.get(column) {
                result.push(value);
            }
        }
    }
    for column in 0..ecc_len {
        for (_, ecc) in &blocks {
            result.push(ecc[column]);
        }
    }
    result
}

fn reed_solomon_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0; degree];
    result[degree - 1] = 1;
    let mut root = 1u8;
    for _ in 0..degree {
        for j in 0..degree {
            result[j] = reed_solomon_multiply(result[j], root);
            if j + 1 < degree {
                result[j] ^= result[j + 1];
            }
        }
        root = reed_solomon_multiply(root, 0x02);
    }
    result
}

fn reed_solomon_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut result = vec![0; divisor.len()];
    for &byte in data {
        let factor = byte ^ result[0];
        result.rotate_left(1);
        *result.last_mut().expect("nonempty QR divisor") = 0;
        for (value, &coefficient) in result.iter_mut().zip(divisor) {
            *value ^= reed_solomon_multiply(coefficient, factor);
        }
    }
    result
}

fn reed_solomon_multiply(mut x: u8, mut y: u8) -> u8 {
    let mut product = 0u8;
    while y != 0 {
        if y & 1 != 0 {
            product ^= x;
        }
        y >>= 1;
        x = (x << 1) ^ (if x & 0x80 != 0 { 0x1D } else { 0 });
    }
    product
}
