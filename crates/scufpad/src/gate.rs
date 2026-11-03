use scufpad_core::{PhysicalControl, PhysicalState};

pub const EXTRAS: u64 = 0x3f << 19;

#[derive(Default)]
pub struct HardwareGate {
    pub verified: u64,
    previous: u64,
    candidate: u64,
}

impl HardwareGate {
    pub fn ready(&self) -> bool {
        self.verified == EXTRAS
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn observe(&mut self, state: PhysicalState) -> bool {
        let previous = self.previous;
        self.previous = state.buttons;
        if state.buttons & !previous & PhysicalControl::HardwareProfile.bit() != 0 {
            self.verified = 0;
            self.candidate = 0;
            return true;
        }
        let extras = state.buttons & EXTRAS;
        let clean =
            state.buttons & !EXTRAS == 0 && state.left_trigger == 0 && state.right_trigger == 0;
        if !clean || extras.count_ones() > 1 {
            self.verified &= !(extras | self.candidate);
            self.candidate = 0;
        } else if extras == 0 {
            self.verified |= self.candidate;
            self.candidate = 0;
        } else if extras & previous == 0 && previous & EXTRAS == 0 {
            self.candidate = extras;
        } else if self.candidate != extras {
            self.candidate = 0;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(buttons: u64) -> PhysicalState {
        PhysicalState {
            buttons,
            ..Default::default()
        }
    }

    #[test]
    fn six_independent_press_release_pairs_are_required() {
        let mut gate = HardwareGate::default();
        for bit in 19..25 {
            gate.observe(state(1 << bit));
            assert!(!gate.ready());
            gate.observe(state(0));
        }
        assert!(gate.ready());
        assert!(gate.observe(state(PhysicalControl::HardwareProfile.bit())));
        assert_eq!(gate.verified, 0);
    }

    #[test]
    fn aliases_during_hold_and_on_release_disqualify() {
        for alias in [1, 1 << 11, 1 << 25, 1 << 29, 1 << 17] {
            let mut gate = HardwareGate::default();
            gate.observe(state(1 << 19));
            gate.observe(state((1 << 19) | alias));
            gate.observe(state(0));
            assert_eq!(gate.verified, 0);
            gate.observe(state(1 << 19));
            gate.observe(state(alias));
            gate.observe(state(0));
            assert_eq!(gate.verified, 0);
        }
    }

    #[test]
    fn triggers_simultaneous_extras_and_reconnect_do_not_credit() {
        let mut gate = HardwareGate::default();
        gate.observe(state(3 << 19));
        gate.observe(state(1 << 19));
        gate.observe(state(0));
        assert_eq!(gate.verified, 0);
        gate.observe(PhysicalState {
            left_trigger: 1,
            ..state(1 << 19)
        });
        gate.observe(state(1 << 19));
        gate.observe(state(0));
        assert_eq!(gate.verified, 0);
        gate.observe(state(1 << 19));
        gate.observe(state(0));
        assert_ne!(gate.verified, 0);
        gate.reset();
        assert_eq!(gate.verified, 0);
    }
}
