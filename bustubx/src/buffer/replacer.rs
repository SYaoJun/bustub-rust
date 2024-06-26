use std::collections::{HashMap, LinkedList};

use super::buffer_pool::FrameId;

#[derive(Debug)]
struct LRUKNode {
    frame_id: FrameId,
    k: usize,
    // 该frame最近k次被访问的时间
    history: LinkedList<u64>,
    // 是否可被置换
    is_evictable: bool,
}
impl LRUKNode {
    pub fn new(frame_id: FrameId, k: usize) -> Self {
        Self {
            frame_id,
            k,
            history: LinkedList::new(),
            is_evictable: false,
        }
    }
    pub fn record_access(&mut self, timestamp: u64) {
        self.history.push_back(timestamp);
        if self.history.len() > self.k {
            self.history.pop_front();
        }
    }
}

#[derive(Debug)]
pub struct LRUKReplacer {
    // 当前可置换的frame数
    current_size: usize,
    // 可置换的frame数上限
    replacer_size: usize,
    k: usize,
    pub node_store: HashMap<FrameId, LRUKNode>,
    // 当前时间戳（从0递增）
    current_timestamp: u64,
}
impl LRUKReplacer {
    pub fn new(num_frames: usize, k: usize) -> Self {
        Self {
            current_size: 0,
            replacer_size: num_frames,
            k,
            node_store: HashMap::new(),
            current_timestamp: 0,
        }
    }

    // 驱逐 evictable 且具有最大 k-distance 的 frame
    pub fn evict(&mut self) -> Option<FrameId> {
        let mut max_k_distance = 0;
        let mut result = None;
        for (frame_id, node) in self.node_store.iter() {
            if !node.is_evictable {
                continue;
            }
            let k_distance = if node.history.len() < self.k {
                u64::MAX - node.history.front().unwrap()
            } else {
                self.current_timestamp - node.history.front().unwrap()
            };
            if k_distance > max_k_distance {
                max_k_distance = k_distance;
                result = Some(*frame_id);
            }
        }
        if result.is_some() {
            self.remove(result.unwrap());
        }
        result
    }

    // 记录frame的访问
    pub fn record_access(&mut self, frame_id: FrameId) {
        if let Some(node) = self.node_store.get_mut(&frame_id) {
            node.record_access(self.current_timestamp);
            self.current_timestamp += 1;
        } else {
            // 创建新node
            assert!(
                self.node_store.len() < self.replacer_size,
                "frame size exceeds the limit"
            );
            let mut node = LRUKNode::new(frame_id, self.k);
            node.record_access(self.current_timestamp);
            self.current_timestamp += 1;
            self.node_store.insert(frame_id, node);
        }
    }

    // 设置frame是否可被置换
    pub fn set_evictable(&mut self, frame_id: FrameId, set_evictable: bool) {
        if let Some(node) = self.node_store.get_mut(&frame_id) {
            let evictable = node.is_evictable;
            node.is_evictable = set_evictable;
            if set_evictable && !evictable {
                self.current_size += 1;
            } else if !set_evictable && evictable {
                self.current_size -= 1;
            }
        } else {
            panic!("frame not found")
        }
    }

    // 移除frame
    pub fn remove(&mut self, frame_id: FrameId) {
        if let Some(node) = self.node_store.get(&frame_id) {
            assert!(node.is_evictable, "frame is not evictable");
            self.node_store.remove(&frame_id);
            self.current_size -= 1;
        }
    }

    // 获取当前可置换的frame数
    pub fn size(&self) -> usize {
        self.current_size
    }
}

#[cfg(test)]
mod tests {
    use super::LRUKReplacer;

    #[test]
    pub fn test_lru_k_set_evictable() {
        let mut replacer = LRUKReplacer::new(3, 2);
        replacer.record_access(1);
        replacer.set_evictable(1, true);
        assert_eq!(replacer.size(), 1);
        replacer.set_evictable(1, false);
        assert_eq!(replacer.size(), 0);
    }

    #[test]
    pub fn test_lru_k_evict_all_pages_at_least_k() {
        let mut replacer = LRUKReplacer::new(2, 3);
        replacer.record_access(1);
        replacer.record_access(2);
        replacer.record_access(2);
        replacer.record_access(1);
        replacer.set_evictable(1, true);
        replacer.set_evictable(2, true);
        let frame_id = replacer.evict();
        assert_eq!(frame_id, Some(1));
    }

    #[test]
    pub fn test_lru_k_evict_some_page_less_than_k() {
        let mut replacer = LRUKReplacer::new(3, 3);
        replacer.record_access(1);
        replacer.record_access(2);
        replacer.record_access(3);
        replacer.record_access(1);
        replacer.record_access(1);
        replacer.record_access(3);
        replacer.set_evictable(1, true);
        replacer.set_evictable(2, true);
        replacer.set_evictable(3, true);
        let frame_id = replacer.evict();
        assert_eq!(frame_id, Some(2));
    }

    #[test]
    pub fn test_lru_k_cmu_bustub_test_case() {
        let mut lru_replacer = LRUKReplacer::new(7, 2);

        // Scenario: add six elements to the replacer. We have [1,2,3,4,5]. Frame 6 is non-evictable.
        lru_replacer.record_access(1);
        lru_replacer.record_access(2);
        lru_replacer.record_access(3);
        lru_replacer.record_access(4);
        lru_replacer.record_access(5);
        lru_replacer.record_access(6);
        lru_replacer.set_evictable(1, true);
        lru_replacer.set_evictable(2, true);
        lru_replacer.set_evictable(3, true);
        lru_replacer.set_evictable(4, true);
        lru_replacer.set_evictable(5, true);
        lru_replacer.set_evictable(6, false);
        assert_eq!(5, lru_replacer.size());

        // Scenario: Insert access history for frame 1. Now frame 1 has two access histories.
        // All other frames have max backward k-dist. The order of eviction is [2,3,4,5,1].
        lru_replacer.record_access(1);

        // Scenario: Evict three pages from the replacer. Elements with max k-distance should be
        // popped first based on LRU.
        let value = lru_replacer.evict();
        assert_eq!(Some(2), value);
        let value = lru_replacer.evict();
        assert_eq!(Some(3), value);
        let value = lru_replacer.evict();
        assert_eq!(Some(4), value);
        assert_eq!(lru_replacer.size(), 2);

        // Scenario: Now replacer has frames [5,1]. Insert new frames 3, 4, and update access
        // history for 5. We should end with [3,1,5,4]
        lru_replacer.record_access(3);
        lru_replacer.record_access(4);
        lru_replacer.record_access(5);
        lru_replacer.record_access(4);
        lru_replacer.set_evictable(3, true);
        lru_replacer.set_evictable(4, true);
        assert_eq!(4, lru_replacer.size());

        // Scenario: continue looking for victims. We expect 3 to be evicted next.
        let value = lru_replacer.evict();
        assert_eq!(Some(3), value);
        assert_eq!(3, lru_replacer.size());

        // Set 6 to be evictable. 6 Should be evicted next since it has max backward k-dist.
        lru_replacer.set_evictable(6, true);
        assert_eq!(4, lru_replacer.size());
        let value = lru_replacer.evict();
        assert_eq!(Some(6), value);
        assert_eq!(3, lru_replacer.size());

        // Now we have [1,5,4]. Continue looking for victims.
        lru_replacer.set_evictable(1, false);
        assert_eq!(2, lru_replacer.size());
        let value = lru_replacer.evict();
        assert_eq!(Some(5), value);
        assert_eq!(1, lru_replacer.size());

        // Update access history for 1. Now we have [4,1]. Next victim is 4.
        lru_replacer.record_access(1);
        lru_replacer.record_access(1);
        lru_replacer.set_evictable(1, true);
        assert_eq!(2, lru_replacer.size());
        let value = lru_replacer.evict();
        assert_eq!(Some(4), value);

        assert_eq!(1, lru_replacer.size());
        let value = lru_replacer.evict();
        assert_eq!(Some(1), value);
        assert_eq!(0, lru_replacer.size());

        // These operations should not modify size
        assert_eq!(None, lru_replacer.evict());
        assert_eq!(0, lru_replacer.size());
        lru_replacer.remove(1);
        assert_eq!(0, lru_replacer.size());
    }
}
