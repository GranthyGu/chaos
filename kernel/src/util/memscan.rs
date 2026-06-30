// KMP-based byte pattern search used by the test/audit subsystem.

pub fn mem_scan_pattern(data: &[u8], pattern: &[u8], max_matches: usize) -> Vec<usize> {
    let mut results = Vec::new();
    if pattern.is_empty() || data.len() < pattern.len() { return results; }
    let plen = pattern.len();
    let mut fail = vec![0usize; plen];
    let mut k = 0;
    for i in 1..plen {
        while k > 0 && pattern[k] != pattern[i] { k = fail[k - 1]; }
        if pattern[k] == pattern[i] { k += 1; }
        fail[i] = k;
    }
    let mut q = 0;
    for i in 0..data.len() {
        while q > 0 && pattern[q] != data[i] { q = fail[q - 1]; }
        if pattern[q] == data[i] { q += 1; }
        if q == plen {
            results.push(i + 1 - plen);
            if results.len() >= max_matches { break; }
            q = fail[q - 1];
        }
    }
    results
}
