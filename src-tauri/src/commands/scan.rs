// TODO: 需要根据新的数据模型重构扫描命令
// 新模型不使用 Scan 结构体，改为使用 Run
// 暂时注释掉所有命令，避免编译错误

/*
/// 开始扫描目录
#[tauri::command]
pub async fn start_scan(
    root_path: String,
    window: Window,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<Run> {
    // TODO: 重新实现
    unimplemented!()
}

/// 暂停扫描
#[tauri::command]
pub async fn pause_scan(
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    // TODO: 重新实现
    unimplemented!()
}

/// 恢复扫描
#[tauri::command]
pub async fn resume_scan(
    run_id: String,
    window: Window,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    // TODO: 重新实现
    unimplemented!()
}

/// 取消扫描
#[tauri::command]
pub async fn cancel_scan(
    run_id: String,
    repo: State<'_, Arc<Mutex<Repository>>>,
) -> Result<()> {
    // TODO: 重新实现
    unimplemented!()
}
*/
