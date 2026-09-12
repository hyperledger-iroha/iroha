impl Drop for ProductionV2Services {
    fn drop(&mut self) {
        let restart_required = !self.clean_teardown;
        if restart_required {
            // Destruction can run before an outer launch permit is released.
            // Reject new work without waiting on that caller's own read lock.
            self.output_guard.close_admission_for_restart();
        }
        self.retire_held_io_completion();
        if let Some(io) = self.io.take()
            && let Err(error) = io.shutdown()
        {
            iroha_logger::error!(%error, "failed to stop Sumeragi v2 I/O worker");
        }
    }
}
