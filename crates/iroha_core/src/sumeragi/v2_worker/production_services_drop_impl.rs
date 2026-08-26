impl Drop for ProductionV2Services {
    fn drop(&mut self) {
        let restart_required = !self.clean_teardown;
        if restart_required {
            self.output_guard.close_admission_for_restart();
        }
        self.retire_held_io_completion();
        if let Some(io) = self.io.take()
            && let Err(error) = io.shutdown()
        {
            iroha_logger::error!(%error, "failed to stop Sumeragi v2 I/O worker");
        }
        if restart_required && !thread::panicking() {
            self.output_guard.activate_restart_required();
        }
    }
}
