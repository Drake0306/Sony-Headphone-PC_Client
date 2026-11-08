//! The Dock icon shows while the main window is open or when the user keeps it.

use shdc_lib::dock::{policy, ActivationPolicyKind};

#[test]
fn dock_policy_follows_main_window() {
    assert_eq!(policy(false, true), ActivationPolicyKind::Accessory);
    assert_eq!(policy(true, true), ActivationPolicyKind::Regular);
    assert_eq!(policy(false, false), ActivationPolicyKind::Regular);
    assert_eq!(policy(true, false), ActivationPolicyKind::Regular);
}
