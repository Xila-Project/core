use crate::{
    Shell,
    error::{Error, Result},
    resolver::resolve,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use xila::{executable::execute, file_system::Path};

impl Shell {
    pub async fn execute<'a, I>(&mut self, input: I, paths: &[&Path]) -> Result<()>
    where
        I: IntoIterator<Item = &'a str>,
    {
        // - Set the current directory for the following commands.
        self.context
            .task_manager
            .set_environment_variable(
                self.task,
                "Current_directory",
                self.current_directory.as_str(),
            )
            .await
            .map_err(Error::FailedToSetCurrentDirectory)?;

        let mut arguments = input.into_iter();

        let path = Path::from_str(arguments.next().ok_or(Error::MissingCommand)?);

        if path.is_valid() {
            if path.is_absolute() {
                self.run(path, arguments).await?;
            } else {
                let path = self
                    .current_directory
                    .clone()
                    .join(path)
                    .ok_or(Error::FailedToJoinPath)?;

                self.run(&path, arguments).await?;
            }
        } else {
            let path = resolve(self.context, path.as_str(), paths).await?;

            self.run(&path, arguments).await?;
        }

        Ok(())
    }

    async fn run<'a, I>(&mut self, path: &Path, arguments: I) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
    {
        let standard = self.standard.duplicate().await.unwrap();

        let arguments: Vec<String> = arguments.into_iter().map(|s| s.to_string()).collect();

        let _ = execute(self.context, path, arguments, standard, None)
            .await
            .map_err(|_| Error::FailedToExecuteCommand)?
            .join()
            .await;

        Ok(())
    }
}
