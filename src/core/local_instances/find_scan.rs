//! 沿用旧版快速扫描：find 查找用户主目录下的 package.json，再读取 JSON 校验实例。
//! 不启动 shell、不做全盘扫描或权限预检；保留安全的路径传输及进程回收。

use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::{Duration, Instant};

use super::scan::{ScanEvent, ScanProgress, ScanReport};
use super::{LocalError, LocalErrorKind, inspect_package, normalized_path};

/// find 的 -path 使用模式匹配，必须额外转义元字符；这不是 shell 转义。
fn literal_pattern(path: &Path) -> OsString {
    let mut escaped = Vec::new();
    for &byte in path.as_os_str().as_bytes() {
        if b"\\*?[]".contains(&byte) {
            escaped.push(b'\\');
        }
        escaped.push(byte);
    }
    OsString::from_vec(escaped)
}

/// 同时排除在线实例的数据卷别名，避免路径入口不同导致遗漏过滤。
fn aliases(path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![path.to_owned()];
    if let Ok(relative) = path.strip_prefix("/System/Volumes/Data") {
        paths.push(Path::new("/").join(relative));
    } else if path.starts_with("/Users") {
        paths.push(Path::new("/System/Volumes/Data").join(path.strip_prefix("/").unwrap_or(path)));
    }
    paths
}

fn build_command(home: &Path, online: &Path) -> Command {
    let mut command = Command::new("/usr/bin/find");
    // 旧版只排除启动器自带实例，其他位置交给系统 find；默认不跟随目录链接。
    let mut online_paths = aliases(online);
    online_paths.extend(aliases(&normalized_path(online)));
    online_paths.sort();
    online_paths.dedup();
    command.arg("-P").arg(home).arg("(");
    for (index, path) in online_paths.iter().enumerate() {
        if index > 0 {
            command.arg("-o");
        }
        command.arg("-path").arg(literal_pattern(path));
    }
    // 用 NUL 代替旧版换行分隔，避免空格、换行等合法路径被拆坏。
    command.args([
        ")",
        "-prune",
        "-o",
        "-type",
        "f",
        "-name",
        "package.json",
        "-print0",
    ]);
    command
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// 分隔符可横跨任意读块；不把文件名当作 UTF-8 文本行处理。
struct Records {
    pending: Vec<u8>,
    delimiter: u8,
    limit: usize,
}
impl Records {
    fn new(delimiter: u8, limit: usize) -> Self {
        Self {
            pending: Vec::new(),
            delimiter,
            limit,
        }
    }
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, LocalError> {
        let mut complete = Vec::new();
        for &byte in bytes {
            if byte == self.delimiter {
                complete.push(std::mem::take(&mut self.pending));
            } else {
                if self.pending.len() >= self.limit {
                    return Err(LocalError::new(
                        "快速扫描输出异常，已停止。",
                        "record too long",
                    ));
                }
                self.pending.push(byte);
            }
        }
        Ok(complete)
    }
}

#[derive(Clone, Copy)]
enum Stream {
    Out,
    Err,
}
enum Output {
    Chunk(Stream, Vec<u8>),
    End(Stream),
    Failed(String),
}

fn read_stream(mut pipe: impl Read, stream: Stream, tx: SyncSender<Output>) {
    let mut buffer = [0; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                let _ = tx.send(Output::End(stream));
                return;
            }
            Ok(count) => {
                if tx
                    .send(Output::Chunk(stream, buffer[..count].to_vec()))
                    .is_err()
                {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = tx.send(Output::Failed(error.to_string()));
                return;
            }
        }
    }
}

/// 可替换的进程控制，测试只注入退出结果，不执行真实扫描。
trait ProcessControl {
    fn poll(&mut self) -> io::Result<Option<i32>>;
    fn stop_and_wait(&mut self);
}
struct FindChild(Child);
impl ProcessControl for FindChild {
    fn poll(&mut self) -> io::Result<Option<i32>> {
        self.0
            .try_wait()
            .map(|status| status.map(|status| status.code().unwrap_or(-1)))
    }
    fn stop_and_wait(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for FindChild {
    fn drop(&mut self) {
        self.stop_and_wait();
    }
}

#[derive(Default)]
struct Diagnostics {
    any: bool,
    recoverable: bool,
    unknown: bool,
    last: String,
}
impl Diagnostics {
    fn observe(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.any = true;
        self.last = String::from_utf8_lossy(bytes).into_owned();
        let recoverable = [
            b": Permission denied".as_slice(),
            b": Operation not permitted",
            b": No such file or directory",
            b": Not a directory",
        ]
        .iter()
        .any(|suffix| bytes.ends_with(suffix));
        self.recoverable |= recoverable;
        self.unknown |= !recoverable;
    }
    fn finish(&self, code: i32, partial: bool) -> Result<bool, LocalError> {
        if code == 0 {
            return Ok(partial || self.any);
        }
        if code > 0 && self.recoverable && !self.unknown {
            return Ok(true);
        }
        Err(LocalError::new(
            "快速扫描命令执行失败。",
            format!("find exit code: {code}\n{}", self.last),
        ))
    }
}

fn cancelled(cancel: &AtomicBool) -> Result<(), LocalError> {
    if cancel.load(Ordering::Relaxed) {
        Err(LocalError::cancelled())
    } else {
        Ok(())
    }
}

fn drive(
    process: &mut impl ProcessControl,
    rx: &Receiver<Output>,
    cancel: &AtomicBool,
    home: &Path,
    online: &Path,
    emit: &mut impl FnMut(ScanEvent) -> bool,
) -> Result<ScanReport, LocalError> {
    let started = Instant::now();
    let mut progress = ScanProgress {
        path: home.display().to_string(),
        ..Default::default()
    };
    let mut paths = Records::new(0, 16 * 1024);
    let mut errors = Records::new(b'\n', 64 * 1024);
    let mut diagnostics = Diagnostics::default();
    let mut partial = false;
    let mut out_done = false;
    let mut err_done = false;
    let mut exit = None;
    let mut last_progress = Instant::now() - Duration::from_secs(1);
    loop {
        cancelled(cancel)?;
        if exit.is_none() {
            exit = process
                .poll()
                .map_err(|e| LocalError::new("无法等待快速扫描进程。", e))?;
        }
        progress.elapsed_seconds = started.elapsed().as_secs();
        if last_progress.elapsed() >= Duration::from_millis(100) {
            if !emit(ScanEvent::Progress(progress.clone())) {
                return Err(LocalError::cancelled());
            }
            last_progress = Instant::now();
        }
        if out_done
            && err_done
            && let Some(code) = exit
        {
            if !paths.pending.is_empty() {
                return Err(LocalError::new(
                    "快速扫描输出异常，已停止。",
                    "missing final NUL",
                ));
            }
            partial = diagnostics.finish(code, partial)?;
            return Ok(ScanReport { progress, partial });
        }
        let output = match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(output) => output,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) if out_done && err_done => {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            Err(_) => {
                return Err(LocalError::new(
                    "快速扫描输出异常，已停止。",
                    "stream disconnected",
                ));
            }
        };
        match output {
            Output::Chunk(Stream::Out, bytes) => {
                for record in paths.push(&bytes)? {
                    cancelled(cancel)?;
                    if record.is_empty() {
                        continue;
                    }
                    let path = PathBuf::from(OsString::from_vec(record));
                    progress.checked += 1;
                    progress.path = path.parent().unwrap_or(&path).display().to_string();
                    if !emit(ScanEvent::ScanningPath(progress.path.clone())) {
                        return Err(LocalError::cancelled());
                    }
                    // 防止异常进程输出进入不属于本轮扫描范围的路径。
                    if !path.starts_with(home)
                        || path.file_name() != Some(OsStr::new("package.json"))
                    {
                        return Err(LocalError::new(
                            "快速扫描输出异常，已停止。",
                            path.display(),
                        ));
                    }
                    if path.to_str().is_none() {
                        partial = true;
                        if !emit(ScanEvent::Warning(LocalError::new(
                            "实例路径无法保存为文本，已跳过。",
                            path.display(),
                        ))) {
                            return Err(LocalError::cancelled());
                        }
                        continue;
                    }
                    match inspect_package(&path, online) {
                        Ok(instance) => {
                            progress.found += 1;
                            if !emit(ScanEvent::Found(instance)) {
                                return Err(LocalError::cancelled());
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind,
                                LocalErrorKind::InvalidInstance | LocalErrorKind::OnlineInstance
                            ) => {}
                        Err(error) => {
                            partial = true;
                            if !emit(ScanEvent::Warning(error)) {
                                return Err(LocalError::cancelled());
                            }
                        }
                    }
                }
            }
            Output::Chunk(Stream::Err, bytes) => {
                for line in errors.push(&bytes)? {
                    diagnostics.observe(&line);
                    if !line.is_empty()
                        && !emit(ScanEvent::Warning(LocalError::new(
                            "快速扫描跳过或遇到错误。",
                            String::from_utf8_lossy(&line),
                        )))
                    {
                        return Err(LocalError::cancelled());
                    }
                }
            }
            Output::End(Stream::Out) => out_done = true,
            Output::End(Stream::Err) => {
                err_done = true;
                if !errors.pending.is_empty() {
                    diagnostics.observe(&errors.pending);
                    if !emit(ScanEvent::Warning(LocalError::new(
                        "快速扫描跳过或遇到错误。",
                        String::from_utf8_lossy(&errors.pending),
                    ))) {
                        return Err(LocalError::cancelled());
                    }
                }
            }
            Output::Failed(error) => return Err(LocalError::new("无法读取快速扫描输出。", error)),
        }
    }
}

/// 测试可以用假进程验证取消和管道失败也会回收，而不启动任何命令。
fn with_cleanup<P: ProcessControl>(
    process: &mut P,
    operation: impl FnOnce(&mut P) -> Result<ScanReport, LocalError>,
) -> Result<ScanReport, LocalError> {
    let result = operation(process);
    process.stop_and_wait();
    result
}

/// 任何退出分支都先终止并回收子进程，再解除管道背压并回收读取线程。
fn execute(
    command: &mut Command,
    home: &Path,
    online: &Path,
    cancel: &AtomicBool,
    emit: &mut impl FnMut(ScanEvent) -> bool,
) -> Result<ScanReport, LocalError> {
    cancelled(cancel)?;
    let mut child = FindChild(
        command
            .spawn()
            .map_err(|error| LocalError::new("无法启动快速扫描命令。", error))?,
    );
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| LocalError::new("无法读取快速扫描输出。", "stdout"))?;
    let stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| LocalError::new("无法读取快速扫描输出。", "stderr"))?;
    let (tx, rx) = mpsc::sync_channel(64);
    let out_tx = tx.clone();
    let out = std::thread::spawn(move || read_stream(stdout, Stream::Out, out_tx));
    let err = std::thread::spawn(move || read_stream(stderr, Stream::Err, tx));
    let result = with_cleanup(&mut child, |child| {
        drive(child, &rx, cancel, home, online, emit)
    });
    drop(rx);
    let out_result = out.join();
    let err_result = err.join();
    if out_result.is_err() || err_result.is_err() {
        return Err(LocalError::new(
            "无法读取快速扫描输出。",
            "reader thread failed",
        ));
    }
    result
}

pub fn run_home(online: PathBuf, cancel: &AtomicBool, mut emit: impl FnMut(ScanEvent) -> bool) {
    let result = (|| {
        cancelled(cancel)?;
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| LocalError::new("无法确定用户主目录，快速扫描未启动。", "HOME"))?;
        // HOME 缺失或异常时不能像旧版一样回退到 /，否则会变成全盘扫描。
        if !home.is_absolute() || home == Path::new("/") {
            return Err(LocalError::new(
                "无法确定用户主目录，快速扫描未启动。",
                home.display(),
            ));
        }
        let home = normalized_path(&home);
        if home == Path::new("/") {
            return Err(LocalError::new(
                "无法确定用户主目录，快速扫描未启动。",
                home.display(),
            ));
        }
        if !emit(ScanEvent::ScanningPath(home.display().to_string())) {
            return Err(LocalError::cancelled());
        }
        let mut command = build_command(&home, &online);
        execute(&mut command, &home, &online, cancel, &mut emit)
    })();
    emit(ScanEvent::Finished(result));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::local_instances::tests::Fixture;

    #[test]
    fn nul_records_handle_chunk_boundaries_and_special_names() {
        let original = b"/home/a b/line\n[?*]\\\xff/package.json";
        let mut decoder = Records::new(0, 1024);
        let mut records = Vec::new();
        for byte in original.iter().chain([0].iter()) {
            records.extend(decoder.push(&[*byte]).unwrap());
        }
        assert_eq!(records, vec![original.to_vec()]);
        let path = PathBuf::from(OsString::from_vec(records.pop().unwrap()));
        assert_eq!(path.as_os_str().as_bytes(), original);
    }

    #[test]
    fn command_matches_legacy_quick_scan_and_only_excludes_online_instance() {
        let home = Path::new("/Users/test [a]?*");
        let online = home.join("online");
        let command = build_command(home, &online);
        assert_eq!(command.get_program(), "/usr/bin/find");
        let args: Vec<_> = command.get_args().map(OsStr::to_owned).collect();
        assert_eq!(
            &args[..2],
            &[OsString::from("-P"), home.as_os_str().to_owned()]
        );
        assert_eq!(args.last().unwrap(), "-print0");
        assert!(args.contains(&literal_pattern(&online)));
        assert!(args.contains(&OsString::from("package.json")));
        assert!(args.contains(&OsString::from("-prune")));
        assert!(!args.contains(&OsString::from("node_modules")));
        assert!(!args.contains(&OsString::from("-exec")));
    }

    #[test]
    fn diagnostic_status_never_hides_nonzero_exit() {
        let mut diagnostics = Diagnostics::default();
        assert!(diagnostics.finish(1, false).is_err());
        diagnostics.observe(b"find: /home/private: Permission denied");
        assert_eq!(diagnostics.finish(1, false).unwrap(), true);
        diagnostics.observe(b"find: unknown option");
        assert!(diagnostics.finish(1, false).is_err());
        assert!(diagnostics.finish(-1, false).is_err());
    }

    struct FakeProcess {
        code: Option<i32>,
        stopped: bool,
    }
    impl ProcessControl for FakeProcess {
        fn poll(&mut self) -> io::Result<Option<i32>> {
            Ok(self.code)
        }
        fn stop_and_wait(&mut self) {
            self.stopped = true;
        }
    }

    #[test]
    fn streams_are_drained_even_after_process_exits() {
        let fixture = Fixture::new();
        let package = fixture.package(r#"{"name":"SILLYTAVERN"}"#);
        let (tx, rx) = mpsc::sync_channel(8);
        tx.send(Output::Chunk(
            Stream::Err,
            b"find: /unavailable: Permission denied\n".to_vec(),
        ))
        .unwrap();
        let mut bytes = package.as_os_str().as_bytes().to_vec();
        bytes.push(0);
        tx.send(Output::Chunk(Stream::Out, bytes)).unwrap();
        tx.send(Output::End(Stream::Out)).unwrap();
        tx.send(Output::End(Stream::Err)).unwrap();
        let mut process = FakeProcess {
            code: Some(1),
            stopped: false,
        };
        let mut found = 0;
        let report = drive(
            &mut process,
            &rx,
            &AtomicBool::new(false),
            &fixture.0,
            &fixture.0.join("online"),
            &mut |event| {
                if matches!(event, ScanEvent::Found(_)) {
                    found += 1;
                }
                true
            },
        )
        .unwrap();
        assert_eq!(found, 1);
        assert_eq!(report.progress.checked, 1);
        assert!(report.partial);
    }

    #[test]
    fn cancellation_and_malformed_output_fail_without_scanning() {
        let (tx, rx) = mpsc::sync_channel(4);
        let mut process = FakeProcess {
            code: Some(0),
            stopped: false,
        };
        assert_eq!(
            drive(
                &mut process,
                &rx,
                &AtomicBool::new(true),
                Path::new("/fixture"),
                Path::new("/online"),
                &mut |_| true
            )
            .unwrap_err()
            .kind,
            LocalErrorKind::Cancelled
        );
        tx.send(Output::Chunk(
            Stream::Out,
            b"/fixture/package.json".to_vec(),
        ))
        .unwrap();
        tx.send(Output::End(Stream::Out)).unwrap();
        tx.send(Output::End(Stream::Err)).unwrap();
        assert!(
            drive(
                &mut process,
                &rx,
                &AtomicBool::new(false),
                Path::new("/fixture"),
                Path::new("/online"),
                &mut |_| true
            )
            .is_err()
        );
    }

    #[test]
    fn stderr_records_have_bounded_memory_under_many_messages() {
        let mut decoder = Records::new(b'\n', 64 * 1024);
        let mut diagnostics = Diagnostics::default();
        for _ in 0..10000 {
            for line in decoder
                .push(b"find: /denied: Operation not permitted\n")
                .unwrap()
            {
                diagnostics.observe(&line);
            }
        }
        assert!(decoder.pending.is_empty());
        assert!(diagnostics.last.len() < 100);
        assert!(diagnostics.finish(1, false).unwrap());
    }
    #[test]
    fn process_is_reaped_on_cancel_and_reader_failure() {
        let (tx, rx) = mpsc::sync_channel(2);
        let mut process = FakeProcess {
            code: None,
            stopped: false,
        };
        let result = with_cleanup(&mut process, |process| {
            drive(
                process,
                &rx,
                &AtomicBool::new(true),
                Path::new("/fixture"),
                Path::new("/online"),
                &mut |_| true,
            )
        });
        assert_eq!(result.unwrap_err().kind, LocalErrorKind::Cancelled);
        assert!(process.stopped);
        process.stopped = false;
        tx.send(Output::Failed("fixture read error".into()))
            .unwrap();
        assert!(
            with_cleanup(&mut process, |process| drive(
                process,
                &rx,
                &AtomicBool::new(false),
                Path::new("/fixture"),
                Path::new("/online"),
                &mut |_| true
            ))
            .is_err()
        );
        assert!(process.stopped);
    }
}
