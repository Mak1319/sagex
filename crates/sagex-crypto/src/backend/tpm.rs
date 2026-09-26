use tss_esapi::{Context, Error, tcti_ldr::TctiNameConf};

pub fn is_available() -> bool {
    let tcti = TctiNameConf::Device(tss_esapi::tcti_ldr::DeviceConfig::default());
    Context::new(tcti)
        .and_then(|mut ctx| ctx.get_random(8))
        .is_ok()
}
