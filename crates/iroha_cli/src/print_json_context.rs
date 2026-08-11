struct PrintJsonContext<W, E> {
    write: W,
    err_write: E,
    config: Config,
    operator_key_pair: Option<KeyPair>,
    transaction_metadata: Option<Metadata>,
    fee_payment: FeePaymentArgs,
    input_instructions: bool,
    output_instructions: bool,
    output_format: CliOutputFormat,
    i18n: Localizer,
}

impl<W: std::io::Write, E: std::io::Write> RunContext for PrintJsonContext<W, E> {
    fn config(&self) -> &Config {
        &self.config
    }

    fn operator_key_pair(&self) -> Option<&KeyPair> {
        self.operator_key_pair.as_ref()
    }

    fn transaction_metadata(&self) -> Option<&Metadata> {
        self.transaction_metadata.as_ref()
    }

    fn transaction_fee_payment(&self) -> Result<FeePaymentIntent> {
        self.fee_payment.selection()
    }

    fn input_instructions(&self) -> bool {
        self.input_instructions
    }

    fn output_instructions(&self) -> bool {
        self.output_instructions
    }

    fn i18n(&self) -> &Localizer {
        &self.i18n
    }

    fn output_format(&self) -> CliOutputFormat {
        self.output_format
    }

    /// Serialize and print data
    ///
    /// # Errors
    ///
    /// - if serialization fails
    /// - if printing fails
    fn print_data<T>(&mut self, data: &T) -> Result<()>
    where
        T: JsonSerialize + ?Sized,
    {
        let mut rendered = norito::json::to_json_pretty(data)
            .map_err(|err| eyre!("failed to render JSON: {err}"))?;
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        self.write.write_all(rendered.as_bytes())?;
        Ok(())
    }

    fn println(&mut self, data: impl Display) -> Result<()> {
        if self.output_format == CliOutputFormat::Json {
            writeln!(&mut self.err_write, "{data}")?;
        } else {
            writeln!(&mut self.write, "{data}")?;
        }
        Ok(())
    }

    fn println_data(&mut self, data: impl Display) -> Result<()> {
        writeln!(&mut self.write, "{data}")?;
        Ok(())
    }
}
