//! The LZO1X compressor of Binarize for version 5 `BMTR` arrays, producing its bytes.
//!
//! An optimal parser: blocks of up to 10,000 bytes, a binary tree match finder over a 2 byte
//! hash, then per block a backward pass over (position, literal state) with a cost in bits
//! per step and a forward pass writing standard LZO1X instructions. The settings are the
//! ones Binarize uses for animations.

use std::cmp::Ordering;

/// Most bytes of a block.
const BLOCK_SIZE: usize = 10_000;
/// Largest distance, `M4_MAX_OFFSET`. Inputs longer than this are compressed by
/// lzokay-native instead, Binarize's window handling for them is not reproduced.
const WINDOW: usize = 0xbfff;
/// Most bytes compared in the tree search.
const SEARCH_LIMIT: usize = 0x400;
/// The tree search stops at a match this long.
const STOP_LENGTH: usize = 0x80;
/// Positions inside a match this long are skipped.
const SKIP_LENGTH: usize = 0x10;
const MIN_LENGTH: usize = 2;
/// Literal states: after a match, after 1, 2, 3 and 4 or more literals.
const STATES: usize = 5;
/// The cost of an impossible step.
const IMPOSSIBLE: u32 = u32::MAX;
/// Distances kept per position, for its longest match and the 7 shorter lengths.
const DISTANCES: usize = 8;

const NONE: u32 = u32::MAX;

/// LZO1X instruction limits.
const M1_MAX_OFFSET: u32 = 0x400;
const M2_MAX_LEN: usize = 8;
const M2_MAX_OFFSET: u32 = 0x800;
const M3_MAX_LEN: usize = 33;
const M3_MAX_OFFSET: u32 = 0x4000;
const M4_MAX_LEN: usize = 9;
const M4_MARKER: u8 = 0x10;
const M3_MARKER: u8 = 0x20;

/// The matches found at a position: the longest and the distances of it and the next shorter
/// lengths.
#[derive(Debug, Clone, Copy, Default)]
struct Matches {
    len: usize,
    distances: [u32; DISTANCES],
}

impl Matches {
    /// The distance used for a match of `len` bytes.
    const fn distance(&self, len: usize) -> u32 {
        let index = self.len - len;
        self.distances[if index < DISTANCES {
            index
        } else {
            DISTANCES - 1
        }]
    }
}

/// Where the tree search attaches the next node.
#[derive(Clone, Copy)]
enum Link {
    /// The left child of the new node (the smaller subtree).
    NewLeft,
    /// The right child of the new node (the larger subtree).
    NewRight,
    Left(usize),
    Right(usize),
}

struct MatchFinder<'a> {
    data: &'a [u8],
    /// Last position of each 2 byte prefix.
    heads: Vec<u32>,
    /// Left (smaller) and right (larger) child per position.
    tree: Vec<[u32; 2]>,
    /// Per length the closest position found by the current search.
    by_length: Vec<u32>,
}

impl<'a> MatchFinder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            heads: vec![NONE; 1 << 16],
            tree: vec![[NONE; 2]; data.len()],
            by_length: vec![0; SEARCH_LIMIT + 2],
        }
    }

    fn set(&mut self, link: Link, new: &mut [u32; 2], value: u32) {
        match link {
            Link::NewLeft => new[0] = value,
            Link::NewRight => new[1] = value,
            Link::Left(p) => self.tree[p][0] = value,
            Link::Right(p) => self.tree[p][1] = value,
        }
    }

    /// Inserts `pos` and returns the length of the longest match, filling `by_length`.
    fn insert(&mut self, pos: usize) -> usize {
        let data = self.data;
        let limit = SEARCH_LIMIT.min(data.len() - pos);
        let hash = usize::from(data[pos]) | usize::from(data[pos + 1]) << 8;
        let candidate = self.heads[hash];
        self.heads[hash] = pos as u32;
        self.tree[pos] = [NONE; 2];
        if candidate == NONE || candidate as usize >= pos {
            return 0;
        }
        self.by_length[MIN_LENGTH] = candidate;

        let mut new = [NONE; 2];
        // Where the next smaller and the next larger node go.
        let mut smaller = Link::NewLeft;
        let mut larger = Link::NewRight;
        let (mut smaller_len, mut larger_len, mut best) = (MIN_LENGTH, MIN_LENGTH, MIN_LENGTH);
        let mut node = candidate as usize;
        loop {
            let mut len = smaller_len.min(larger_len);
            let (here, there) = loop {
                let (here, there) = (data[pos + len], data[node + len]);
                if here != there {
                    break (here, there);
                }
                len += 1;
                if len >= limit {
                    break (here, there);
                }
            };
            match here.cmp(&there) {
                Ordering::Less => {
                    // The node is larger: it goes right, continue in its left subtree.
                    if len > larger_len {
                        if len > best {
                            self.by_length[best + 1..=len].fill(node as u32);
                            best = len;
                            if len >= STOP_LENGTH {
                                self.set(larger, &mut new, node as u32);
                                self.set(smaller, &mut new, NONE);
                                break;
                            }
                        }
                        larger_len = len;
                    }
                    self.set(larger, &mut new, node as u32);
                    larger = Link::Left(node);
                    let next = self.tree[node][0];
                    if next == NONE {
                        self.set(smaller, &mut new, NONE);
                        break;
                    }
                    node = next as usize;
                }
                Ordering::Greater => {
                    if len > smaller_len {
                        if len > best {
                            self.by_length[best + 1..=len].fill(node as u32);
                            best = len;
                            if len >= STOP_LENGTH {
                                self.set(smaller, &mut new, node as u32);
                                self.set(larger, &mut new, NONE);
                                break;
                            }
                        }
                        smaller_len = len;
                    }
                    self.set(smaller, &mut new, node as u32);
                    smaller = Link::Right(node);
                    let next = self.tree[node][1];
                    if next == NONE {
                        self.set(larger, &mut new, NONE);
                        break;
                    }
                    node = next as usize;
                }
                Ordering::Equal => {
                    // Equal up to the limit: the new node replaces this one.
                    let end = len.max(best + 1);
                    self.by_length[best + 1..=end].fill(node as u32);
                    best = len;
                    let [left, right] = self.tree[node];
                    self.set(smaller, &mut new, left);
                    self.set(larger, &mut new, right);
                    break;
                }
            }
        }
        self.tree[pos] = new;
        best
    }

    /// Finds the matches at `pos`, `remaining` bytes before the end of the block.
    fn matches(&mut self, pos: usize, remaining: usize) -> Matches {
        let mut found = Matches::default();
        if pos + 3 > self.data.len() {
            self.tree[pos] = [NONE; 2];
            return found;
        }
        let len = self.insert(pos).min(remaining);
        if len < MIN_LENGTH {
            return found;
        }
        found.len = len;
        let distance = |l: usize| (pos - self.by_length[l] as usize) as u32;
        found.distances[0] = distance(len);
        for k in 1..DISTANCES {
            if len - k < MIN_LENGTH {
                break;
            }
            found.distances[k] = distance(len - k);
        }
        if len == SEARCH_LIMIT {
            // Extended past the search limit with the closest match.
            let d = found.distances[0] as usize;
            let data = self.data;
            let mut end = len;
            while end < remaining && data[pos + end] == data[pos + end - d] {
                end += 1;
            }
            if end > len {
                let extra = end - len;
                let old = found.distances;
                for k in 0..DISTANCES {
                    found.distances[k] = if k < extra { d as u32 } else { old[k - extra] };
                }
                found.len = end;
            }
        }
        found
    }

    /// A position inside a long match: not inserted.
    fn skip(&mut self, pos: usize) {
        self.tree[pos] = [NONE; 2];
    }
}

/// A step of the backward pass: its cost to the end of the block and the match length
/// taken, 0 for a literal.
#[derive(Debug, Clone, Copy, Default)]
struct Step {
    cost: u32,
    len: usize,
}

/// Bits of a match of `len` bytes at `distance` from literal state `state`, `None` if it
/// can not be encoded there.
fn match_bits(len: usize, distance: u32, state: usize) -> Option<u32> {
    if len < MIN_LENGTH {
        return None;
    }
    if len == 2 {
        return (distance <= M1_MAX_OFFSET && (1..=3).contains(&state)).then_some(16);
    }
    if len <= M2_MAX_LEN && distance <= M2_MAX_OFFSET {
        return Some(16);
    }
    if len == 3 && distance <= M2_MAX_OFFSET + 0x400 && state > 3 {
        return Some(16);
    }
    let max_short = if distance <= M3_MAX_OFFSET {
        M3_MAX_LEN
    } else {
        M4_MAX_LEN
    };
    if len <= max_short {
        return Some(24);
    }
    let rest = len - max_short;
    let mut bits = 32;
    if rest > 255 {
        bits += 8 * (1 + (rest - 256) / 255) as u32;
    }
    Some(bits)
}

struct Encoder<'a> {
    data: &'a [u8],
    out: Vec<u8>,
    /// Literals not written yet.
    pending: usize,
}

impl Encoder<'_> {
    /// Writes the `pending` literals before `pos`.
    fn literals(&mut self, pos: usize) {
        let count = self.pending;
        if count == 0 {
            return;
        }
        let out = &mut self.out;
        if out.is_empty() && count < 0xef {
            out.push((count + 17) as u8);
        } else if count <= 3 {
            let at = out.len() - 2;
            out[at] |= count as u8;
        } else if count <= 18 {
            out.push((count - 3) as u8);
        } else {
            out.push(0);
            push_length(out, count - 18);
        }
        out.extend_from_slice(&self.data[pos - count..pos]);
    }

    fn write_match(&mut self, len: usize, distance: u32) {
        let literals = self.pending;
        self.pending = 0;
        let out = &mut self.out;
        if len == 2 {
            let d = distance - 1;
            out.extend([((d & 3) << 2) as u8, (d >> 2) as u8]);
        } else if len <= M2_MAX_LEN && distance <= M2_MAX_OFFSET {
            let d = distance - 1;
            out.extend([
                (((len - 1) << 5) as u32 | (d & 7) << 2) as u8,
                (d >> 3) as u8,
            ]);
        } else if len == 3 && distance <= M2_MAX_OFFSET + 0x400 && literals > 3 {
            let d = distance - M2_MAX_OFFSET - 1;
            out.extend([((d & 3) << 2) as u8, (d >> 2) as u8]);
        } else {
            let d = if distance <= M3_MAX_OFFSET {
                if len <= M3_MAX_LEN {
                    out.push(M3_MARKER | (len - 2) as u8);
                } else {
                    out.push(M3_MARKER);
                    push_length(out, len - M3_MAX_LEN);
                }
                distance - 1
            } else {
                let d = distance - M3_MAX_OFFSET;
                let high = ((d >> 11) & 8) as u8;
                if len <= M4_MAX_LEN {
                    out.push(M4_MARKER | high | (len - 2) as u8);
                } else {
                    out.push(M4_MARKER | high);
                    push_length(out, len - M4_MAX_LEN);
                }
                d
            };
            out.extend([((d & 63) << 2) as u8, (d >> 6) as u8]);
        }
    }
}

/// A length past the instruction's: a zero byte per 255, then the rest.
fn push_length(out: &mut Vec<u8>, mut rest: usize) {
    while rest > 255 {
        out.push(0);
        rest -= 255;
    }
    out.push(rest as u8);
}

/// Compresses `data` like Binarize.
pub(super) fn compress(data: &[u8]) -> Vec<u8> {
    let n = data.len();
    if n > WINDOW {
        return lzokay_native::compress(data).unwrap_or_default();
    }
    let mut encoder = Encoder {
        data,
        out: Vec::with_capacity(n / 2 + 16),
        pending: 0,
    };
    if n >= 4 {
        compress_blocks(data, &mut encoder);
    } else {
        encoder.pending = n;
    }
    encoder.literals(n);
    encoder.out.extend([M4_MARKER | 1, 0, 0]);
    encoder.out
}

fn compress_blocks(data: &[u8], encoder: &mut Encoder) {
    let n = data.len();
    let block_size = BLOCK_SIZE.min(n);
    let mut finder = MatchFinder::new(data);
    let mut matches = vec![Matches::default(); block_size + 1];
    let mut steps = vec![Step::default(); (block_size + 1) * STATES];
    let (mut start, mut searched_end, mut searched_len) = (0, 0, 0);
    while start < n {
        let len = (n - start).min(block_size);
        // Positions searched for the last block but cut off it are kept.
        let from = if searched_end > start {
            let kept = searched_end - start;
            matches.copy_within(searched_len - kept..searched_len, 0);
            searched_end
        } else {
            start
        };
        let end = start + len;
        let mut pos = from;
        while pos < end {
            let found = finder.matches(pos, end - pos);
            matches[pos - start] = found;
            if found.len >= SKIP_LENGTH {
                for skipped in pos + 1..pos + found.len {
                    matches[skipped - start] = Matches::default();
                    finder.skip(skipped);
                }
                pos += found.len;
            } else {
                pos += 1;
            }
        }
        searched_end = end;
        searched_len = end - start;

        if start == 0 {
            for m in matches.iter_mut().take(4.min(end)) {
                if m.len == 2 {
                    m.len = 0;
                }
            }
        }
        let cut = if end == n {
            end
        } else {
            cut_point(&matches, start, end)
        };
        plan(&matches, &mut steps, cut - start);
        encode(&matches, &steps, start, cut, encoder);
        start = cut;
    }
}

/// Where Binarize ends a block: the last position after `start` no match reaches over that
/// has no match itself, else the last one with a match below 3 bytes, else `end`.
fn cut_point(matches: &[Matches], start: usize, end: usize) -> usize {
    let (mut free, mut short, mut reach) = (start, start, start);
    for pos in start..end {
        let len = matches[pos - start].len;
        if pos == reach {
            if len == 0 {
                free = pos;
            } else if len < 3 {
                short = pos;
            }
        }
        reach = reach.max(pos + len.max(1));
    }
    if free != start {
        free
    } else if short != start {
        short
    } else {
        end
    }
}

/// The backward pass over the `len` positions of a block.
fn plan(matches: &[Matches], steps: &mut [Step], len: usize) {
    steps[len * STATES..(len + 1) * STATES].fill(Step::default());
    for p in (0..len).rev() {
        let m = matches[p];
        let (gap_start, keep) = if m.len < SKIP_LENGTH {
            (M2_MAX_LEN, 4)
        } else {
            (MIN_LENGTH + 1, 1)
        };
        let resume = if gap_start + keep < m.len {
            m.len - keep
        } else {
            gap_start
        };
        for state in (0..STATES).rev() {
            let next = (state + 1).min(STATES - 1);
            let mut best = Step {
                cost: steps[(p + 1) * STATES + next]
                    .cost
                    .wrapping_add(if state == 3 { 16 } else { 8 }),
                len: 0,
            };
            let mut l = MIN_LENGTH;
            while l <= m.len {
                if l == gap_start {
                    l = resume;
                }
                let cost = match_bits(l, m.distance(l), state).map_or(IMPOSSIBLE, |bits| {
                    steps[(p + l) * STATES].cost.wrapping_add(bits)
                });
                if cost <= best.cost {
                    best = Step { cost, len: l };
                }
                l += 1;
            }
            steps[p * STATES + state] = best;
        }
    }
}

/// The forward pass: writes the instructions of the block `start..end`.
fn encode(matches: &[Matches], steps: &[Step], start: usize, end: usize, encoder: &mut Encoder) {
    let mut pos = start;
    while pos < end {
        let state = encoder.pending.min(STATES - 1);
        let step = steps[(pos - start) * STATES + state];
        if step.len == 0 {
            encoder.pending += 1;
            pos += 1;
        } else {
            encoder.literals(pos);
            encoder.write_match(step.len, matches[pos - start].distance(step.len));
            pos += step.len;
        }
    }
}
