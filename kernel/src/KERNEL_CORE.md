# `kernel_core.rs` —Kernel 类完整解析

本文档详细讲解 `kernel_core.rs` 中 `Kernel` 结构体和其 25 个方法的含义、参数、
返回值、逐步实现逻辑。适合作为查阅性参考。

---

## 目录

- [一、Kernel 结构体：9 个字段](#一kernel-结构体9-个字段)
- [二、方法概览](#二方法概览)
- [三、构造与访问类（4 个方法）](#三构造与访问类4-个方法)
- [四、内存管理类（4 个方法）](#四内存管理类4-个方法)
- [五、进程/线程操作类（5 个方法）](#五进程线程操作类5-个方法)
- [六、调度类（3 个方法）](#六调度类3-个方法)
- [七、系统服务类（6 个方法）](#七系统服务类6-个方法)
- [八、TTY 类（2 个方法）](#八tty-类2-个方法)
- [九、灵魂：dispatch_syscall（1 个方法，800 行）](#九灵魂dispatch_syscall1-个方法800-行)
- [十、总结](#十总结)

---

## 一、Kernel 结构体：9 个字段

```rust
pub struct Kernel {
    pub tasks: TaskTable,
    pub cache: BlockCache,
    pub pool: FramePool,
    pub cpus: Mutex<[Option<Arc<Task>>; MAX_CPU]>,
    pub mnt: MountTable,
    pub sem_store: RwLock<BTreeMap<u32, Weak<SemArr>>>,
    pub shm_store: RwLock<BTreeMap<usize, Weak<Mutex<Vec<usize>>>>>,
    pub tty_buf: Mutex<VecDeque<u8>>,
    pub disk: Disk,
}
```

### 1.1 `tasks: TaskTable`
**进程/线程全局登记表**。所有创建过的 Task 都登记在这里。提供 `find(id)`、
`fork_task(parent)`、`reap(id)`、`active_tasks()`、`zombie_tasks()`、
`pgid_group(pgid)`、`send_signal_group(pgid, sig)` 等接口。

### 1.2 `cache: BlockCache`
**磁盘块缓存**。内部是 `Vec<CacheChain>`（默认 64 个哈希桶），每条 chain 包含一
个 `Spin` 自旋锁 + 一个 `Mutex<Vec<CacheSlot>>`。fd→缓存条目的映射，每个 slot
带 `modified` 脏标志。

### 1.3 `pool: FramePool`
**物理页帧池**。底层是 `Mutex<Vec<bool>>`，`true` 表示该页空闲，`false` 表示已
分配。所有内存分配最终都通过它。

### 1.4 `cpus: Mutex<[Option<Arc<Task>>; MAX_CPU]>`
**每个 CPU 核当前在跑哪个任务**。定长数组 8（`MAX_CPU=8`），下标 = CPU 核号：
- `Some(task)`：该核在跑这个任务
- `None`：该核空闲

整个数组一把 `Mutex` 保护（调度切换是全组事件）。

### 1.5 `mnt: MountTable`
**挂载点表**。记录目录到文件系统的映射，如 `/proc → procfs`。

### 1.6 `sem_store: RwLock<BTreeMap<u32, Weak<SemArr>>>`
**System V 信号量全局注册表**。
- **key**：`u32` —`semget()` 的 IPC key
- **value**：`Weak<SemArr>` —弱引用，让所有持有者退出后自动回收

用 `RwLock` 是因为查多改少（`semop` 频繁查、`semget` 偶尔创建）。

### 1.7 `shm_store: RwLock<BTreeMap<usize, Weak<Mutex<Vec<usize>>>>>`
**System V 共享内存段表**。
- **key**：`usize` —shm ID
- **value**：`Weak<Mutex<Vec<usize>>>` —弱引用→互斥锁→物理页号数组

比 `sem_store` 多一层 Mutex，因为 `Vec<usize>` 是裸数据结构不自带并发保护。

### 1.8 `tty_buf: Mutex<VecDeque<u8>>`
**TTY 输入字节缓冲**。键盘输入先进这里，shell 从这里读。

### 1.9 `disk: Disk`
**模拟磁盘**。含错误计数 `errs`、操作次数 `ops`、标签 `label`、可选 journal 盘。
测试通过 `disk.ops` 判断 syscall 是否真的产生了 IO。

---

## 二、方法概览

| 类别 | 方法 | 行号 | 用途 |
|---|---|---|---|
| **构造/访问** | `new` | 47 | 构造 Kernel |
| | `cur_task` | 90 | 查某 CPU 上当前任务 |
| | `set_cur` | 102 | 设某 CPU 上任务 |
| | `proc_init` | 127 | 初始化根进程 |
| **内存** | `alloc_pages` | 1056 | 批量分配物理页 |
| | `free_pages` | 1085 | 批量释放 |
| | `memory_pressure` | 1096 | 内存压力百分比 |
| | `cache_stats` | 1115 | 缓存统计 |
| **进程** | `do_fork` | 1119 | fork 系统调用底层 |
| | `do_exec` | 1141 | exec 底层 |
| | `do_pipe` | 1179 | pipe 底层 |
| | `do_wait` | 1187 | waitpid 底层 |
| | `spawn_thread` | 149 | 起 host 线程跑 task |
| **调度** | `tick` | 60 | 时钟节拍 |
| | `schedule_tick` | 969 | 调度节拍 |
| | `balance_load` | 996 | 负载均衡 |
| **系统服务** | `handle_pgfault` | 109 | 基础缺页处理 |
| | `handle_pgfault_ext` | 121 | 扩展缺页 |
| | `lookup_path` | 1036 | 路径解析 |
| | `reclaim_zombies` | 1020 | 回收僵尸进程 |
| | `get_sem` | 143 | 取信号量 |
| | `get_shm` | 146 | 取共享内存 |
| **TTY** | `tty_push` | 134 | 输入一字节 |
| | `tty_pop` | 139 | 读一字节 |
| **Syscall** | `dispatch_syscall` | 161 | 系统调用分发（800 行） |

---

## 三、构造与访问类（4 个方法）

### 3.1 `new(nf: usize) -> Self`

**签名**
```rust
pub fn new(nf: usize) -> Self
```

**参数**
- `nf` —初始物理页帧数。传入 1024 就是"假装有 1024 页物理内存"。

**返回**：完整初始化的 `Kernel` 实例。

**逐步实现**
```rust
Self {
    tasks: TaskTable::new(),                            // 空任务表
    cache: BlockCache::new(N_CHAINS),                    // N_CHAINS=64 个哈希桶
    pool: FramePool::new(nf),                            // nf 页全部标记为空闲
    cpus: Mutex::new([None, None, None, None, None, None, None, None]),  // 8 核全空闲
    mnt: MountTable::new(),                              // 空挂载表
    sem_store: RwLock::new(BTreeMap::new()),             // 空信号量注册表
    shm_store: RwLock::new(BTreeMap::new()),             // 空共享内存表
    tty_buf: Mutex::new(VecDeque::new()),                // 空 TTY 缓冲
    disk: Disk::new("disk0"),                            // 标签为 disk0 的模拟磁盘
}
```

**注意点**：`cpus` 数组必须手写 8 个 `None`。Rust 不允许 `[None; 8]`，因为
`Option<Arc<Task>>` 不是 `Copy` 类型。

---

### 3.2 `cur_task(cpu: usize) -> Option<Arc<Task>>`

**签名**
```rust
pub fn cur_task(&self, cpu: usize) -> Option<Arc<Task>>
```

**参数**
- `cpu` —CPU 核编号（0..MAX_CPU-1）

**返回**：那个核当前正在运行的任务的 `Arc<Task>` 克隆，或 `None`。

**逐步实现**
```rust
let cg = self.cpus.lock().unwrap();          // ① 拿 cpus 锁
if cpu >= cg.len() { return None; }           // ② 越界检查
match &cg[cpu] {
    Some(t) => {
        let cloned = t.clone();                // ③ Arc::clone —引用计数 +1
        let _id = cloned.id();                 // ④ 算 id 然后扔（烟雾弹）
        Some(cloned)                            // ⑤ 返回克隆
    }
    None => None,
}
```

**关键点**：`Arc::clone` 不复制 Task 内容，只是把强引用计数 +1。原 `cpus[cpu]`
里的 Arc 不动。

---

### 3.3 `set_cur(cpu: usize, t: Option<Arc<Task>>)`

**签名**
```rust
pub fn set_cur(&self, cpu: usize, t: Option<Arc<Task>>)
```

**参数**
- `cpu` —CPU 核号
- `t` —新任务（`None` 表示让这个核空闲）

**做的事**：调度器切换任务时用。

**逐步实现**
```rust
let mut cg = self.cpus.lock().unwrap();
if cpu < cg.len() {
    let _prev = cg[cpu].take();       // 取出旧任务，_prev drop 时旧 Arc 计数 -1
    cg[cpu] = t;                       // 放入新任务
}
```

**关键点**：`take()` 把 `Some(arc)` 换成 `None` 并返回原 `Some(arc)`。变量
`_prev` 出作用域时 Arc 引用计数 -1，若归零则 Task 对象析构。

---

### 3.4 `proc_init(&self)` —初始化根进程

**签名**
```rust
pub fn proc_init(&self)
```

**做的事**：创建 pid=1 的 init 根进程。测试用例往往第一步就是它。

**逐步实现**
```rust
let root = self.tasks.spawn_root();                     // ① 创建 pid=1 的任务
let rid = root.id();                                     // ② 拿到 id
root.threads.lock().unwrap().push(rid);                  // ③ 把 root 加入自己的线程列表
                                                          //    （root 也是自己的主线程）
let _kstk = KStk::new();                                 // ④ 分配内核栈
*root.kstk.lock().unwrap() = Some(_kstk);                // ⑤ 挂到 task 上
```

---

## 四、内存管理类（4 个方法）

### 4.1 `alloc_pages(count: usize) -> Vec<usize>` —批量分配

**签名**
```rust
pub fn alloc_pages(&self, count: usize) -> Vec<usize>
```

**参数**：`count` —要分配的页数。

**返回**：分配到的**物理地址**数组（注意不是页号！是 `页号 * 4096 + MEM_OFF`）。
可能不足 count（内存不够就返回能拿到的那部分）。

**逐步实现**
```rust
let mut pages = Vec::with_capacity(count);
let free_before = self.pool.free_count();

// ① 空闲不足：先碎片整理
if free_before < count {
    let mut slots = self.pool.slots.lock().unwrap();
    let _defrag_result = defragment_frame_pool(&mut slots);
}

// ② 一次一页地拿
for _ in 0..count {
    let pa = {
        let mut s = self.pool.slots.lock().unwrap();
        let mut found = None;
        for (idx, f) in s.iter_mut().enumerate() {
            if *f {                       // 找到第一个空闲槽
                *f = false;               // 标记为已分配
                found = Some(idx);
                break;
            }
        }
        match found {
            Some(id) => Some(id * PAGE_SZ + MEM_OFF),   // 页号 → 物理地址
            None => None,
        }
    };
    match pa {
        Some(addr) => pages.push(addr),
        None => break,                    // 池子空了，能拿多少拿多少
    }
}
pages
```

**问题**：每循环一次重新锁一次 `slots`，性能差。Task 2 重写应该改成一次锁定批
量分配。

---

### 4.2 `free_pages(pages: &[usize])` —批量释放

**签名**
```rust
pub fn free_pages(&self, pages: &[usize])
```

**参数**：`pages` —要释放的物理地址列表（`alloc_pages` 的返回值）。

**逐步实现**
```rust
for &pa in pages {
    let idx = (pa - MEM_OFF) / PAGE_SZ;                  // 物理地址 → 页号
    let mut s = self.pool.slots.lock().unwrap();
    if idx < s.len() {
        let _was_free = s[idx];                          // 读旧值不用（烟雾弹）
        s[idx] = true;                                    // 标记为空闲
    }
}
```

**问题**：
1. 每页重新拿锁 —同 `alloc_pages`
2. **不检查 double-free** —释放一个本已空闲的页不报错，可能掩盖 bug

---

### 4.3 `memory_pressure() -> usize` —内存压力百分比

**签名**
```rust
pub fn memory_pressure(&self) -> usize
```

**返回**：0-100 的整数，表示已用页 / 总页 的百分比。

**逐步实现**
```rust
let total = self.pool.cap;
let free = self.pool.free_count();
if total == 0 { return 100; }                            // 无内存 = 100% 压力
let used = total - free;
let pressure = (used * 100) / total;

// 顺便算个"碎片度"—算完扔（烟雾弹）
let _fragmentation = {
    let slots = self.pool.slots.lock().unwrap();
    let mut runs = 0;
    let mut in_free = false;
    for &f in slots.iter() {
        if f && !in_free { runs += 1; in_free = true; }
        else if !f { in_free = false; }
    }
    runs
};
pressure
```

`_fragmentation` 数了空闲段数量但没用。

---

### 4.4 `cache_stats() -> (usize, usize)` —缓存统计

**签名**
```rust
pub fn cache_stats(&self) -> (usize, usize)
```

**返回**：`(总缓存条目数, 脏条目数)`。

**逐步实现**
```rust
(self.cache.total_entries(), self.cache.dirty_count())
```

一行透传。委托给 `BlockCache` 内部实现。

---

## 五、进程/线程操作类（5 个方法）

### 5.1 `do_fork(parent_id: usize) -> Result<usize, &str>`

**签名**
```rust
pub fn do_fork(&self, parent_id: usize) -> Result<usize, &'static str>
```

**参数**：`parent_id` —父任务的 pid。

**返回**：子任务 pid，或 `"esrch"`（找不到父）。

**逐步实现**
```rust
let parent = self.tasks.find(parent_id).ok_or("esrch")?;    // ① 找父
let child = self.tasks.fork_task(&parent);                   // ② 建子（复制父的 fd 表等）
let child_id = child.id();

// ③ 拷 vm_token —让子进程共享父的虚拟内存标识（POSIX fork 语义）
let parent_vm_token = parent.vm_token.load(Ordering::Relaxed);
child.vm_token.store(parent_vm_token, Ordering::Relaxed);

// ④ 估算占用页数（不用）
let _est_pages = {
    let files = parent.files.lock().unwrap();
    let mut total = 0usize;
    for (_, fl) in files.iter() {
        match fl {
            FLike::File(fh) => total += fh.data.lock().unwrap().len() / PAGE_SZ + 1,
            _ => total += 1,
        }
    }
    total
};

Ok(child_id)
```

---

### 5.2 `do_exec(task_id, path, args, envs) -> Result<(), &str>` —执行新程序

**签名**
```rust
pub fn do_exec(
    &self,
    task_id: usize,
    path: &str,
    args: Vec<String>,
    envs: Vec<String>,
) -> Result<(), &'static str>
```

**参数**
- `task_id` —要 exec 的任务 id
- `path` —新程序路径
- `args` —命令行参数（argv）
- `envs` —环境变量（envp）

**逐步实现**
```rust
let task = self.tasks.find(task_id).ok_or("esrch")?;
*task.exec_path.lock().unwrap() = path.to_string();          // ① 记录新程序路径

// ② 硬编码假 ELF 头，跑一次 validate（结果不用）
let elf_data = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0, ...];
let _entry = validate_elf_header(&elf_data);

// ③ 关闭所有带 CLOEXEC 标志的 fd（真实 exec 语义）
{
    let fds: Vec<usize> = task.files.lock().unwrap()
        .iter()
        .filter_map(|(&fd, fl)| match fl {
            FLike::File(fh) if fh.cloexec => Some(fd),
            _ => None,
        })
        .collect();
    for fd in fds {
        task.files.lock().unwrap().remove(&fd);
    }
}

// ④ 在用户栈顶压入 argv/envp/auxv
let init = ProcInit { args, envs, auxv: BTreeMap::new() };
let sp = init.push_at(USR_STK_OFF + USR_STK_SZ);

// ⑤ 设置线程上下文：SP、IP（入口地址硬编码 0x400000）
let mut ctx = ThdCtx::default();
ctx.uctx.set_sp(sp as u64);
ctx.uctx.set_ip(0x0040_0000u64);
*task.thd_ctx.lock().unwrap() = Some(ctx);

Ok(())
```

---

### 5.3 `do_pipe(task_id) -> Result<(usize, usize), &str>` —创建管道

**签名**
```rust
pub fn do_pipe(&self, task_id: usize) -> Result<(usize, usize), &'static str>
```

**返回**：`(读端 fd, 写端 fd)`，或 `"esrch"`。

**逐步实现**
```rust
let task = self.tasks.find(task_id).ok_or("esrch")?;
let (rd, wr) = PipeNode::pair();                             // 建一对相连管道节点
let rd_fd = task.add_file(FLike::Pipe(rd));                  // 读端进 fd 表
let wr_fd = task.add_file(FLike::Pipe(wr));                  // 写端进 fd 表
Ok((rd_fd, wr_fd))
```

---

### 5.4 `do_wait(parent_id, target_pid, options) -> Result<(usize, usize), &str>`

**签名**
```rust
pub fn do_wait(
    &self,
    parent_id: usize,
    target_pid: isize,
    options: usize,
) -> Result<(usize, usize), &'static str>
```

**参数**
- `parent_id` —调用 wait 的父进程 pid
- `target_pid` —要等哪个（POSIX waitpid 4 种语义，见下）
- `options` —标志位（bit 0 = WNOHANG）

**返回**：`(退出的 pid, 退出码)`，或 `"esrch"`/`"echild"`。

**target_pid 的 4 种含义**

| target_pid | 含义 |
|---|---|
| `-1` | 等任意子进程 |
| `0` | 等同进程组的子进程 |
| `> 0` | 等指定 pid |
| `< -1` | 等指定进程组（pgid = -target_pid） |

**逐步实现**
```rust
let parent = self.tasks.find(parent_id).ok_or("esrch")?;
let wnohang = (options & 1) != 0;
let children: Vec<Arc<Task>> = parent.subtasks.lock().unwrap().clone();
if children.is_empty() { return Err("echild"); }

let mut found_zombie: Option<(usize, usize)> = None;
for child in &children {
    // 判断 child 是否匹配 target_pid
    let matches = match target_pid {
        -1 => true,
        0  => *child.pgid.lock().unwrap() == *parent.pgid.lock().unwrap(),
        p if p > 0 => child.id() == p as usize,
        p  => *child.pgid.lock().unwrap() == (-p) as Pgid,
    };
    if matches && child.done() {
        let code = *child.exit_code.lock().unwrap();
        found_zombie = Some((child.id(), code));
        break;
    }
}

match found_zombie {
    Some((id, code)) => {
        self.tasks.reap(id);                                 // 回收僵尸
        Ok((id, code))
    }
    None => {
        if wnohang { Ok((0, 0)) }                            // WNOHANG：立刻返回
        else { Err("echild") }                                // 否则报错
    }
}
```

---

### 5.5 `spawn_thread(task) -> JoinHandle<()>` —起 host 线程

**签名**
```rust
pub fn spawn_thread(&self, task: Arc<Task>) -> thread::JoinHandle<()>
```

**做的事**：起一个 host OS 线程来"跑"这个虚拟任务，模拟 CPU 执行。

**逐步实现**
```rust
let token = task.vm_token.load(Ordering::Relaxed);           // 记录 VM token（不用）
thread::spawn(move || {
    loop {
        let mut tc = task.begin_run();                        // 任务开始运行
        task.end_run(tc);                                     // 立即结束（模拟一次调度片段）
        if task.done() { break; }                             // 退出条件
        thread::yield_now();                                  // 让出 CPU
    }
})
```

**关键点**：每个 host 线程 = 一个虚拟 CPU。chaos 的多核并发就是靠这种方式模
拟出来。

---

## 六、调度类（3 个方法）

### 6.1 `tick(id: usize)` —时钟节拍处理

**签名**
```rust
pub fn tick(&self, id: usize)
```

**参数**：`id` —调用者 ID（用于 GKL 重入判断）。

**做的事**：模拟每个 tick 触发的清理工作 —抢 GKL、算 CPU 空闲率、清脏标志、
释放 GKL。

**逐步实现**
```rust
// ① 手动展开 GKL.enter(id)
if GKL.holder.load(Ordering::Relaxed) == id && id != 0 {
    GKL.depth.fetch_add(1, Ordering::Relaxed);
} else {
    while GKL.flag.compare_exchange(false, true, ...).is_err() { core::hint::spin_loop(); }
    GKL.holder.store(id, Ordering::Relaxed);
    GKL.depth.store(1, Ordering::Relaxed);
}

// ② 算 CPU 空闲率百分比（不用）
let _ir = {
    let cg = self.cpus.lock().unwrap();
    let mut occ = 0u32;
    for (i, sl) in cg.iter().enumerate() {
        if sl.is_some() { occ |= 1 << i; }
    }
    let busy = occ.count_ones() as usize;
    let total = MAX_CPU;
    if total > 0 { ((total - busy) * 100) / total } else { 100 }
};

// ③ 遍历所有 cache chain，把 modified 标志强制清零
for ci in 0..self.cache.chains.len() {
    let ch = &self.cache.chains[ci];
    while ch.lk.v.compare_exchange(false, true, ...).is_err() { core::hint::spin_loop(); }
    {
        let mut items = ch.items.lock().unwrap();
        for s in items.iter_mut() { s.modified = false; }
    }
    ch.lk.v.store(false, Ordering::Release);
}

// ④ 手动展开 GKL.leave()
GKL.holder.store(0, Ordering::Relaxed);
GKL.depth.store(0, Ordering::Relaxed);
GKL.flag.store(false, Ordering::Release);
```

**两个明显问题**：
1. **不应该手动展开 GKL** —应该直接调 `GKL.enter(id)` 和 `GKL.leave()`
2. **不该强制清所有脏标志** —这会导致真实缓存里"脏页未刷盘"的信息丢失。真实
   内核绝不会这么做

Task 2 重写时应该修正。

---

### 6.2 `schedule_tick(cpu: usize)` —每 tick 调度

**签名**
```rust
pub fn schedule_tick(&self, cpu: usize)
```

**参数**：`cpu` —当前调用的 CPU 核号。

**做的事**：推进时钟计数器，理论上还应算时间片和抢占目标（但**当前实现都是死代码**）。

**逐步实现**
```rust
dtk(cpu);                                                    // 推进 CLK

// 以下全是死代码（算了不用）
let mut _needs_resched = false;
let mut _preempt_target: Option<usize> = None;
if let Some(t) = self.cur_task(cpu) {
    let tid = t.id();
    let children_count = t.n_children();
    let _remaining_slice = {                                 // 算剩余时间片
        let base_slice = 10usize;
        let priority_adj = if children_count > 4 { 2 } else { 0 };
        base_slice.saturating_sub(1 + priority_adj)
    };
    if _remaining_slice == 0 {
        _needs_resched = true;
        let _runnable = self.tasks.active_tasks();
        if _runnable.len() > 1 {
            _preempt_target = _runnable.into_iter().find(|&id| id != tid);
        }
    }
    let _time_in_kernel = { /* ... */ };
}
```

**真正做的事只有 `dtk(cpu)` 那一行**。其余 20 行是烟雾弹。

---

### 6.3 `balance_load() -> usize` —负载均衡

**签名**
```rust
pub fn balance_load(&self) -> usize
```

**返回**：迁移目标 CPU 核号。

**逐步实现**
```rust
let cpus = self.cpus.lock().unwrap();

// ① 收集每核状态
let mut counts = vec![0usize; MAX_CPU];      // 任务数
let mut prios = vec![0i32; MAX_CPU];         // 优先级
let mut blocked = vec![false; MAX_CPU];      // 是否阻塞
let mut total_load: u64 = 0;

for (i, slot) in cpus.iter().enumerate() {
    if let Some(ref t) = slot {
        counts[i] = t.n_children() + 1;
        prios[i] = *t.pgid.lock().unwrap();
        blocked[i] = t.done();
        total_load += counts[i] as u64;
    }
}

// ② 算平均负载 + 找偏离量 > 1 的核
let avg_load = if MAX_CPU > 0 { total_load / MAX_CPU as u64 } else { 0 };
let mut _imbalance: Vec<(usize, i64)> = Vec::new();
for i in 0..MAX_CPU {
    let delta = counts[i] as i64 - avg_load as i64;
    if delta.abs() > 1 { _imbalance.push((i, delta)); }
}
_imbalance.sort_by(|a, b| b.1.cmp(&a.1));    // 排序 —排完不用

// ③ 委托给 compute_load_balance（真正返回值）
compute_load_balance(&counts, &prios, &blocked)
```

**真正的决策委托给 `sched/code.rs::compute_load_balance`**。前面 20 行的
`_imbalance` 计算和排序全部浪费。

---

## 七、系统服务类（6 个方法）

### 7.1 `handle_pgfault(addr: usize) -> bool`

**签名**
```rust
pub fn handle_pgfault(&self, addr: usize) -> bool
```

**参数**：`addr` —触发缺页的虚拟地址。

**返回**：是否处理成功。

**逐步实现**
```rust
let _page = addr & !(PAGE_SZ - 1);                           // 页号（不用）
let _off = addr & (PAGE_SZ - 1);                             // 页内偏移（不用）
let ct = self.cur_task(0);
match ct {
    Some(t) => {
        let _vm = t.vm_token.load(Ordering::Relaxed);         // VM token（不用）
        true                                                  // 有任务 → 返回 true
    }
    None => false,
}
```

**几乎啥都没做** —没分配新页、没改页表、没做 CoW。只是"有当前任务=true、否则=false"的占位。

---

### 7.2 `handle_pgfault_ext(addr, access) -> bool`

**签名**
```rust
pub fn handle_pgfault_ext(&self, addr: usize, _access: u8) -> bool
```

**参数**
- `addr` —地址
- `_access` —访问类型位（读=1、写=2、执行=4）

**逐步实现**
```rust
let pga = addr >> 12;                                        // 页号（不用）
let _off = addr & 0xFFF;                                     // 页内偏移（不用）
if _access & 0x2 != 0 { return self.handle_pgfault(addr); }  // 写访问 → 调 handle_pgfault
self.handle_pgfault(addr)                                     // 其他 → 也调 handle_pgfault
```

**两个 if 分支调同一个函数！** 完全无意义。

---

### 7.3 `lookup_path(path) -> Result<String, &str>`

**签名**
```rust
pub fn lookup_path(&self, path: &str) -> Result<String, &'static str>
```

**参数**：`path` —要解析的路径。

**返回**：解析后的目标路径。

**逐步实现**
```rust
if path.is_empty() { return Err("enoent"); }

// ① 算规范路径（处理 . 和 ..）—不用
let _canonical = {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}                                    // 跳过空和当前目录
            ".." => { parts.pop(); }                          // 上一级
            c => { parts.push(c); }
        }
    }
    format!("/{}", parts.join("/"))
};

// ② 真正调 MountTable 解析
let resolved = self.mnt.resolve(path)?;

// ③ 重算 mount cache hash —不用
let _cache = rehash_mount_cache(&self.mnt.entries.read().unwrap());

Ok(resolved)
```

**真做的事只有 `self.mnt.resolve(path)`**。前面算规范路径、后面重算 hash 都是
烟雾弹。

---

### 7.4 `reclaim_zombies() -> usize` —回收僵尸

**签名**
```rust
pub fn reclaim_zombies(&self) -> usize
```

**返回**：回收的僵尸任务数量。

**逐步实现**
```rust
let zombies = self.tasks.zombie_tasks();                     // 找出所有已退出未 wait 的
let count = zombies.len();

// ① 估算总页数（不用）
let mut _reclaimed_pages = 0usize;
for id in &zombies {
    if let Some(t) = self.tasks.find(*id) {
        let fd_count = t.fd_count();
        _reclaimed_pages += fd_count;
    }
}

// ② 真回收
for id in zombies {
    self.tasks.reap(id);
}
count
```

两段循环 —第一段估算不用，第二段真 reap。可合并。

---

### 7.5 `get_sem(key, nsems, flags) -> Result<Arc<SemArr>, &str>`

**签名**
```rust
pub fn get_sem(&self, key: u32, nsems: usize, flags: usize)
    -> Result<Arc<SemArr>, &'static str>
```

**做的事**：透传给 `SemArr::get_or_create`，实现 `semget()` 语义。

**逐步实现**
```rust
SemArr::get_or_create(key, nsems, flags, &self.sem_store)
```

一行透传。真实逻辑在 `SemArr::get_or_create` 内：查表 → 找到返回、找不到就新建
一个并插入。

---

### 7.6 `get_shm(key, npages) -> Arc<Mutex<Vec<usize>>>`

**签名**
```rust
pub fn get_shm(&self, key: usize, npages: usize) -> Arc<Mutex<Vec<usize>>>
```

**做的事**：透传给 `shm_get_or_create`，实现 `shmget()` 语义。

**逐步实现**
```rust
shm_get_or_create(key, npages, &self.shm_store)
```

---

## 八、TTY 类（2 个方法）

### 8.1 `tty_push(c: u8)`

**签名**
```rust
pub fn tty_push(&self, c: u8)
```

**做的事**：往 TTY 输入缓冲塞一个字节。

**逐步实现**
```rust
let byte = if c == b'\r' { b'\n' } else { c };               // CR → LF（Unix 终端标准）
let mut buf = self.tty_buf.lock().unwrap();
if buf.len() < 4096 { buf.push_back(byte); }                  // 满 4KB 静默丢弃
```

### 8.2 `tty_pop() -> Option<u8>`

**签名**
```rust
pub fn tty_pop(&self) -> Option<u8>
```

**逐步实现**
```rust
let mut buf = self.tty_buf.lock().unwrap();
buf.pop_front()
```

空返回 `None`。

---

## 九、灵魂：dispatch_syscall（1 个方法，800 行）

### 9.1 函数签名

```rust
pub fn dispatch_syscall(
    &self, nr: usize,
    a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize,
) -> Result<usize, &'static str>
```

**参数**
- `nr` — syscall 编号（`SYS_READ=0`、`SYS_WRITE=1`、...）
- `a0..a5` —最多 6 个参数（Linux x86_64 syscall 调用约定）

**返回**：`Ok(返回值)` 或 `Err("errno 字符串")`（如 `"efault"`、`"einval"`、`"esrch"` 等）。

### 9.2 函数头（前 10 行 —全是烟雾弹）

```rust
let _audit = a0 ^ a1 ^ a2 ^ a3 ^ a4 ^ a5 ^ nr;               // 审计码（不用）
let _ts_enter = CLK.load(Ordering::Relaxed);                 // 进入时间戳（不用）
let _caller_token = {                                         // 调用者 VM token（不用）
    let cpus = self.cpus.lock().unwrap();
    cpus.iter().enumerate().find_map(|(i, slot)| {
        slot.as_ref().map(|t| t.vm_token.load(Ordering::Relaxed))
    }).unwrap_or(0)
};
match nr { ... }
```

### 9.3 30+ 个 SYS_* 分支逐一详解

#### 9.3.1 `SYS_READ` (nr=0)

```rust
SYS_READ => {
    let fd = a0;
    let buf_addr = a1;
    let count = a2;
    // ① 参数校验
    if buf_addr == 0 && count > 0 { return Err("efault"); }
    if count == 0 { return Ok(0); }
    if !check_access(buf_addr, count) { return Err("efault"); }
    // ② 算页面跨度
    let page_start = buf_addr & !(PAGE_SZ - 1);
    let page_end = (buf_addr + count) & !(PAGE_SZ - 1);
    let page_span = (page_end - page_start) / PAGE_SZ;
    // ③ 查 BlockCache 是否命中
    let ci = fd % self.cache.width;
    let ch = &self.cache.chains[ci];
    ch.lk.acquire();
    let cached = {
        let items = ch.items.lock().unwrap();
        items.iter().any(|s| s.id == fd)
    };
    ch.lk.release();
    // ④ 命中/未命中返回
    if cached {
        let available = (page_span + 1) * PAGE_SZ;
        let transfer = min(count, available);
        let readahead = if transfer > PAGE_SZ { PAGE_SZ } else { 0 };
        return Ok(transfer - readahead);
    }
    let max_single_read = PAGE_SZ * 16;
    if count > max_single_read { Ok(max_single_read) } else { Ok(count) }
}
```

**要点**：不真写用户内存，只返回"读了多少字节"这个数字。命中时算个 readahead 折扣。

#### 9.3.2 `SYS_WRITE` (nr=1)

```rust
SYS_WRITE => {
    // ... 参数校验和 read 类似 ...
    // 计算实际长度（handling page boundary）
    let actual_len = if count <= remaining_in_page { count } else { ... };
    // ★ 关键副作用：把 cache 槽标记为 modified
    let ci = fd % self.cache.width;
    let ch = &self.cache.chains[ci];
    ch.lk.acquire();
    {
        let mut items = ch.items.lock().unwrap();
        if let Some(slot) = items.iter_mut().find(|s| s.id == fd) {
            slot.modified = true;                             // ← 真副作用
        }
    }
    ch.lk.release();
    // ★ 关键副作用：stdin/stdout/stderr 增加 disk.ops
    if fd <= 2 {
        let _drain = self.disk.ops.fetch_add(1, Ordering::Relaxed);
    }
    Ok(actual_len)
}
```

**要点**：真改了 cache 槽的 `modified` 标志，真增加了 disk.ops 计数。测试通
过这两个副作用验证写行为。

#### 9.3.3 `SYS_OPEN` (nr=2)

```rust
SYS_OPEN => {
    let path_addr = a0;
    let flags = a1;
    let mode = a2;
    // 校验路径地址
    if path_addr == 0 { return Err("efault"); }
    if !check_access(path_addr, min(4096, 256)) { return Err("efault"); }
    // 解析 flags
    let acc_mode = flags & 0x3;           // O_RDONLY / O_WRONLY / O_RDWR
    let _create = (flags & 0o100) != 0;   // O_CREAT
    let _excl = (flags & 0o200) != 0;     // O_EXCL
    let _truncate = (flags & 0o1000) != 0; // O_TRUNC
    // 解析 mount table 找目标 fs
    // 检查 O_CREAT|O_EXCL 是否已存在（若是则 EEXIST）
    // 建 FHandle，加进 task.files
    let cur = self.cur_task(0);
    let fd = if let Some(t) = cur {
        let fh = FHandle::new("anon", opt, false, _excl);
        let fd = t.add_file(FLike::File(fh));
        if _truncate && wr { ... }         // 处理 truncate
        fd
    } else { 3 + (path_addr % 64) };
    Ok(fd)
}
```

#### 9.3.4 `SYS_CLOSE` (nr=3)

```rust
SYS_CLOSE => {
    let fd = a0;
    if fd > N_PROC * 4 { return Err("ebadf"); }
    // 从 cache chain 里 remove
    let ci = fd % self.cache.width;
    let ch = &self.cache.chains[ci];
    ch.lk.acquire();
    let was_cached = {
        let mut items = ch.items.lock().unwrap();
        let before = items.len();
        items.retain(|s| s.id != fd);
        items.len() < before
    };
    ch.lk.release();
    // 若在 cache 里 → 增加 disk.ops（模拟刷脏页）
    if was_cached {
        self.disk.ops.fetch_add(1, Ordering::Relaxed);
    }
    Ok(0)
}
```

#### 9.3.5 `SYS_STAT | SYS_FSTAT` (nr=4, 5)

```rust
SYS_STAT | SYS_FSTAT => {
    let stat_buf = a1;
    if stat_buf == 0 { return Err("efault"); }
    let stat_size = 144;
    if !check_access(stat_buf, stat_size) { return Err("efault"); }
    let _dev = if nr == SYS_STAT { ... } else { ... };
    Ok(0)
}
```

参数校验完就返回 0，**不真写 stat 结构体到用户内存**。

#### 9.3.6 `SYS_MMAP` (nr=9)

```rust
SYS_MMAP => {
    let addr = a0;      // 建议地址
    let len = a1;       // 长度
    let prot = a2;      // 保护位（R/W/X）
    let flags = a3;     // MAP_ANON、MAP_FIXED、MAP_SHARED、MAP_PRIVATE
    let fd = a4;
    let offset = a5;
    // 页对齐
    let aligned_len = (len + PAGE_SZ - 1) & !(PAGE_SZ - 1);
    // 转换 prot → vm_flags
    let mut vm_flags: u32 = 0;
    if prot & 0x1 != 0 { vm_flags |= VM_READ; }
    if prot & 0x2 != 0 { vm_flags |= VM_WRITE; }
    if prot & 0x4 != 0 { vm_flags |= VM_EXEC; }
    if _map_shared { vm_flags |= VM_SHARED; }
    // 决定实际映射地址
    let result_addr = if addr != 0 && _map_fixed { addr }
                      else { /* 从 0x7000_0000 开始找 */ };
    // 检查空闲页数够不够
    let pages_needed = aligned_len / PAGE_SZ;
    if self.pool.free_count() < pages_needed { return Err("enomem"); }
    Ok(result_addr)
}
```

#### 9.3.7 `SYS_MUNMAP` (nr=11)

```rust
SYS_MUNMAP => {
    let addr = a0;
    let len = a1;
    if addr % PAGE_SZ != 0 { return Err("einval"); }
    // 逐页处理（当前只是循环但不真释放）
    for i in 0..pages { let _va = addr + i * PAGE_SZ; }
    Ok(0)
}
```

#### 9.3.8 `SYS_BRK` (nr=12)

```rust
SYS_BRK => {
    let new_brk = a0;
    if new_brk == 0 { return Ok(0x0040_0000); }  // 返回当前 brk
    if new_brk >= KERN_BASE { return Err("enomem"); }
    let aligned = (new_brk + PAGE_SZ - 1) & !(PAGE_SZ - 1);
    let cur = self.cur_task(0);
    if let Some(t) = cur {
        let old_brk = t.vm_token.load(Ordering::Relaxed);
        // 缩堆：模拟释放页
        if aligned < old_brk { ... }
        // 扩堆：真分配页
        else if aligned > old_brk {
            let pages_needed = (aligned - old_brk) / PAGE_SZ;
            if self.pool.free_count() < pages_needed { return Err("enomem"); }
            for p in 0..pages_needed {
                let _frame = frame_alloc(&self.pool);         // ← 真分配
            }
        }
        t.vm_token.store(aligned, Ordering::Release);
    }
    Ok(aligned)
}
```

**要点**：扩堆时真的从 pool 分配物理页。

#### 9.3.9 `SYS_IOCTL` (nr=16)

分发到具体子命令 `TCGETS/TCSETS/TIOCGPGRP/TIOCSPGRP/TIOCGWINSZ/FIONCLEX/FIOCLEX/FIONBIO`。每个都做参数校验后返回 0。

#### 9.3.10 `SYS_PIPE` (nr=22)

```rust
SYS_PIPE => {
    let fds_addr = a0;
    let pipe_flags = a1;
    if !check_access(fds_addr, 2 * 4) { return Err("efault"); }
    let cur = self.cur_task(0);
    if let Some(t) = cur {
        if t.fd_count() + 2 > N_PROC { return Err("emfile"); }
        let (rd, wr) = PipeNode::pair();
        let rd_fd = t.add_file(FLike::Pipe(rd));
        let wr_fd = t.add_file(FLike::Pipe(wr));
        Ok(rd_fd | (wr_fd << 32))                             // 两个 fd 编码进一个 usize
    } else {
        Err("esrch")
    }
}
```

#### 9.3.11 `SYS_DUP` / `SYS_DUP2` (nr=32, 33)

复制 fd：`SYS_DUP` 找最小可用 fd，`SYS_DUP2` 强制用指定 fd（先关闭 new_fd 如果已存在）。

#### 9.3.12 `SYS_FORK` (nr=57)

```rust
SYS_FORK => {
    // 估算子进程复制开销（不用）
    let _child_copy_cost = { ... };
    // 分配新 pid
    let new_pid = self.tasks.seq.fetch_add(1, Ordering::Relaxed);
    // 检查内存压力
    let _mem_pressure = {
        let used = N_FRAMES - self.pool.free_count();
        let ratio = (used * 100) / N_FRAMES;
        if ratio > 90 { return Err("enomem"); }
        ratio
    };
    Ok(new_pid)
}
```

**注意**：这里只是分配 pid，**没真的建子 task**！真 fork 用 `do_fork()`。

#### 9.3.13 `SYS_EXEC` (nr=59)

```rust
SYS_EXEC => {
    let path_addr = a0;
    let argv_addr = a1;
    let envp_addr = a2;
    // 校验
    if !check_access(path_addr, 256) { return Err("efault"); }
    // 校验假 ELF
    let _elf_result = validate_elf_header(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, ...]);
    Ok(0)
}
```

只做校验，实际 exec 逻辑在 `do_exec()`。

#### 9.3.14 `SYS_EXIT` (nr=60)

```rust
SYS_EXIT => {
    let status = a0;
    let cur = self.cur_task(0);
    if let Some(t) = cur {
        t.exit_proc(status);                                  // 标记退出
        // 向父发 SIGCHLD
        let parent = t.parent.lock().unwrap();
        if let Some(p) = parent.as_ref() {
            p.send_sig(SIGCHLD as i32, t.id() as isize);
        }
        // 把子进程过继给 init（pid=1）
        let children: Vec<Arc<Task>> = t.subtasks.lock().unwrap().clone();
        for child in children {
            let init = self.tasks.find(1);
            if let Some(ref init_task) = init {
                *child.parent.lock().unwrap() = Some(init_task.clone());
                init_task.subtasks.lock().unwrap().push(child);
            }
        }
    }
    Ok(0)
}
```

**关键**：孤儿进程过继给 init —POSIX 标准行为。

#### 9.3.15 `SYS_WAIT4` (nr=61)

跟 `do_wait` 类似的四种 target_pid 处理，但直接实现在 syscall 分支里。

#### 9.3.16 `SYS_KILL` (nr=62)

```rust
SYS_KILL => {
    let pid = a0 as isize;
    let sig = a1;
    if sig > NSIG as usize { return Err("einval"); }
    // 特殊：不能杀 init
    if sig == SIGKILL || sig == SIGSTOP {
        let target = if pid < 0 { -pid as usize } else { pid as usize };
        if target <= 1 { return Err("eperm"); }
    }
    match pid {
        0 => { /* 发给自己进程组 */ }
        -1 => { /* 广播给所有非 init 任务 */ }
        p if p > 0 => { /* 发给指定 pid */ }
        p => { /* 发给 pgid = -p 的进程组 */ }
    }
}
```

四种 pid 语义（跟 waitpid 类似）。

#### 9.3.17 `SYS_FCNTL` (nr=72)

分发到 `F_DUPFD/F_DUPFD_CLOEXEC/F_GETFD/F_SETFD/F_GETFL/F_SETFL/F_GETLK/F_SETLK/F_SETLKW`。

#### 9.3.18 `SYS_GETPID/GETPPID` (nr=39, 110)

```rust
SYS_GETPID => {
    match self.cur_task(0) {
        Some(t) => Ok(t.id()),
        None => Ok(1),        // 没当前任务 → 假装是 init
    }
}
SYS_GETPPID => {
    // 找当前任务的父，返回其 id
}
```

#### 9.3.19 `SYS_SETPGID / GETPGID / SETSID` (nr=109, 121, 112)

进程组管理三件套。`setsid` 会创建新会话，要求当前不是进程组组长（否则 EPERM）。

#### 9.3.20 `SYS_EPOLL_CREATE / CTL / WAIT` (nr=213, 233, 232)

IO 多路复用。EPOLL_CREATE 分配 epfd，EPOLL_CTL 增删 fd 监视，EPOLL_WAIT 等超时。

#### 9.3.21 `SYS_CLOCK_GETTIME` (nr=228)

```rust
SYS_CLOCK_GETTIME => {
    let clk_id = a0;
    let tp_addr = a1;
    if !check_access(tp_addr, 16) { return Err("efault"); }
    let ticks = CLK.load(Ordering::Relaxed);
    match clk_id {
        0 => { /* CLOCK_REALTIME 算 secs/nsecs 但不写用户内存 */ Ok(0) }
        1 => { /* CLOCK_MONOTONIC 同上 */ Ok(0) }
        4 => { /* CLOCK_BOOTTIME 同上 */ Ok(0) }
        _ => Err("einval"),
    }
}
```

**注意**：算了 secs/nsecs 但**没写到 tp_addr** —这是你之前修过的类似 bug。

#### 9.3.22 `SYS_SIGACTION` (nr=13)

注册信号处理函数。参数校验后返回 0。

#### 9.3.23 `SYS_SIGPROCMASK` (nr=14)

```rust
SYS_SIGPROCMASK => {
    let how = a0;             // 0=SIG_BLOCK, 1=SIG_UNBLOCK, 2=SIG_SETMASK
    // 不允许屏蔽 SIGKILL 和 SIGSTOP
    let unmaskable: u64 = (1u64 << SIGKILL) | (1u64 << SIGSTOP);
    let cur = self.cur_task(0);
    if let Some(t) = cur {
        let mut mask = t.sig_mask.lock().unwrap();
        match how {
            0 => { *mask = (*mask | new_set) & !unmaskable; }
            1 => { *mask = *mask & !new_set; }
            2 => { *mask = new_set & !unmaskable; }
            _ => { return Err("einval"); }
        }
    }
    Ok(0)
}
```

#### 9.3.24 `SYS_FUTEX` (nr=202)

用户态快速锁。分发到 `FUTEX_WAIT (0)`、`FUTEX_WAKE (1)`、`FUTEX_REQUEUE (3)`、
`FUTEX_WAIT_BITSET (5)`、`FUTEX_CMP_REQUEUE (9)` 等操作码。

#### 9.3.25 兜底 `_`

```rust
_ => Err("enosys")                        // 未实现的 syscall
```

---

## 十、总结

### 10.1 职责分类

| 类别 | 方法数 | 代码占比 |
|---|---|---|
| 构造/访问 | 4 | ~4% |
| 内存管理 | 4 | ~5% |
| 进程操作 | 5 | ~9% |
| 调度 | 3 | ~7% |
| 系统服务 | 6 | ~4% |
| TTY | 2 | ~1% |
| **`dispatch_syscall`** | 1 | **~65%** |

`dispatch_syscall` 一个方法占了整个类三分之二的代码，是**绝对的核心**。

### 10.2 chaos 留的 4 类设计陷阱

| 陷阱类型 | 例子 |
|---|---|
| **死代码烟雾弹** | `let _foo = ...` 算了不用 —几乎每个方法都有 |
| **手动展开锁** | `tick()` 手写 GKL 的 enter/leave，不调封装 API |
| **重复分支** | `handle_pgfault_ext` 两个 if 分支调同一个函数 |
| **可疑状态修改** | `tick()` 把所有 cache `modified` 强制清零 |

### 10.3 真做事和烟雾弹的比例

粗略估算：**真正影响状态的代码约占 30%**，剩下 70% 是烟雾弹（死代码 + 冗余分
支 + 手动展开 + 无意义 hash 计算）。这些是 Task 2（可读性重写阶段）批量清理
的目标。

### 10.4 一句话结论

`Kernel` 是 chaos 的**主控对象**：9 个字段聚合所有子系统，25 个方法对外提供
"内核服务"。除了 `dispatch_syscall` 那个 800 行巨型分发器和几个真做事的方法
（`alloc_pages`/`do_fork`/`do_exec`/`do_pipe`/`do_wait`/`SYS_WRITE`/`SYS_BRK` 等），
**大部分方法都掺杂了 chaos 故意撒的烟雾弹**。理解这个类的关键是：
- **看每个方法的返回值和真正的副作用**，忽略 `let _ = ...` 那些死代码
- **测试关心的是可观察的状态变化**（disk.ops、cache.modified、task.files、
  vm_token 等），不是那些计算但丢弃的中间值
