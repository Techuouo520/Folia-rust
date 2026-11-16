// Refreshing the single app QR cancels old work and ignores replies from its previous round.
use folia_omni::{LoginQr, Omni};
use folia_ui::{Login, LoginMessage, Online};
use iced::futures::StreamExt;

#[test]
fn refreshing_qq_login_cancels_the_previous_request() {
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::new(temp.path().into(), "folia-login-method-test".into()).unwrap();
    let (online, restore) = Online::new(Some(omni.clone()), Some("netease"));
    drop(restore);
    let mut login = Login::new(std::time::Instant::now());
    let old = login.open(Some("qq"), &online, Some(&omni));
    assert!(old.units() > 0);
    let (fresh, _) = login.update(LoginMessage::Retry, Some(&omni), &online, false);
    assert!(fresh.units() > 0);
    let mut old = iced_runtime::task::into_stream(old).unwrap();
    assert!(pollster::block_on(old.next()).is_none());
    let stale = LoginQr { key: "old".into(), width: 21, modules: vec![false; 441], browser: None };
    let (ignored, _) = login.update(LoginMessage::Qr(3, Ok(stale)), Some(&omni), &online, false);
    assert_eq!(ignored.units(), 0);
    let (same, _) = login.update(LoginMessage::Method("qqmusic"), Some(&omni), &online, false);
    assert_eq!(same.units(), 0);
    let (unknown, _) = login.update(LoginMessage::Method("wechat"), Some(&omni), &online, false);
    assert_eq!(unknown.units(), 0);
    login.close();
    let mut fresh = iced_runtime::task::into_stream(fresh).unwrap();
    assert!(pollster::block_on(fresh.next()).is_none());
}
