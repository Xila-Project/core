#![no_std]

extern crate alloc;
#[cfg(test)]
extern crate std;

use crate::{Error, Result};
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::Write;
use core::num::NonZeroUsize;
use error::*;
use xila::file_system::Path;
use xila::task;
use xila::{executable, file_system::PathOwned};
use xila::{
    executable::{ExecutableTrait, Standard},
    task::TaskIdentifier,
};

mod commands;
mod error;
//mod parser;
mod resolver;
//mod tokenizer;

use commands::{CommandContext, dispatch_user_command, resolve_user_command};

pub struct Shell {
    context: &'static executable::ExecutableContext,
    task: TaskIdentifier,
    standard: Standard<'static>,
    current_directory: PathOwned,
    running: bool,
    user: String,
    host: String,
}

pub struct ShellExecutable;

struct ShellCommandContext<'a> {
    shell: &'a mut Shell,
}

impl<'a> ShellCommandContext<'a> {
    fn new(shell: &'a mut Shell) -> Self {
        Self { shell }
    }
}

impl CommandContext for ShellCommandContext<'_> {
    fn executable_context(&self) -> &'static executable::ExecutableContext {
        self.shell.context
    }
    fn task_id(&self) -> TaskIdentifier {
        self.shell.task
    }

    fn current_directory(&self) -> &Path {
        &self.shell.current_directory
    }

    fn set_current_directory(&mut self, directory: PathOwned) {
        self.shell.current_directory = directory;
    }

    fn stop(&mut self) {
        self.shell.running = false;
    }

    fn write_out_fmt(&mut self, arguments: core::fmt::Arguments<'_>) -> Result<()> {
        self.shell
            .standard
            .out()
            .write_fmt(arguments)
            .map_err(Into::into)
    }

    async fn write_out(&mut self, buffer: &[u8]) {
        let _ = self.shell.standard.out().write(buffer).await;
    }

    async fn write_out_line(&mut self, buffer: &[u8]) {
        let _ = self.shell.standard.out().write_line(buffer).await;
    }

    fn standard(&mut self) -> &mut Standard<'static> {
        &mut self.shell.standard
    }
}

impl ExecutableTrait for ShellExecutable {
    fn main(standard: Standard<'static>, arguments: Vec<String>) -> executable::MainFuture {
        Box::pin(async move { main(standard, arguments).await })
    }
}

pub async fn main(
    standard: Standard<'static>,
    arguments: Vec<String>,
) -> core::result::Result<(), NonZeroUsize> {
    Shell::new(standard).await.main(arguments).await
}

impl Shell {
    pub async fn new(standard: Standard<'static>) -> Self {
        let context = standard.context;
        Self {
            context,
            standard,
            task: context.task_manager.get_current_task_identifier().await,
            current_directory: Path::ROOT.to_owned(),
            running: true,
            user: "".to_string(),
            host: "".to_string(),
        }
    }

    async fn parse_input<'a, I>(&mut self, input: I, paths: &[&Path]) -> Result<()>
    where
        I: IntoIterator<Item = &'a str> + Clone,
    {
        let mut options = getargs::Options::new(input.clone().into_iter());

        let next_positional = match options.next_positional() {
            Some(arg) => arg,
            None => return Ok(()),
        };

        let result = match resolve_user_command(next_positional) {
            Some(user_command) => {
                let mut context = ShellCommandContext::new(self);
                dispatch_user_command(user_command, &mut context, &mut options, paths).await
            }
            None => self.execute(input, paths).await,
        };

        if let Err(error) = result {
            writeln!(self.standard.standard_error, "{}", error)?;
        }

        Ok(())
    }

    async fn main_interactive(&mut self, paths: &[&Path]) -> Result<()> {
        let mut input_string = String::with_capacity(64);

        while self.running {
            let _ = write!(
                self.standard.out(),
                "{}@{}:{}$ ",
                self.user,
                self.host,
                self.current_directory
            );

            let _ = self.standard.out().flush().await;

            input_string.clear();

            self.standard.read_line(&mut input_string).await.unwrap();

            if input_string.is_empty() {
                continue;
            }

            let input = input_string.split(" ");

            let result = self.parse_input(input, paths).await;

            if let Err(error) = result {
                let _ = writeln!(self.standard.standard_error, "{}", error);
            }
        }

        Ok(())
    }

    pub async fn main(&mut self, arguments: Vec<String>) -> core::result::Result<(), NonZeroUsize> {
        let task_manager = &self.context.task_manager;
        let task = task_manager.get_current_task_identifier().await;

        let user = match task_manager.get_environment_variable(task, "User").await {
            Ok(user) => user.get_value().to_string(),
            Err(_) => loop {
                match self.authenticate().await {
                    Ok(user) => break user,
                    Err(error) => {
                        let _ = writeln!(self.standard.standard_error, "{}", error);
                    }
                }
            },
        };

        self.user = user;

        let paths = task_manager
            .get_environment_variable(task, "Paths")
            .await
            .map_err(|_| Error::FailedToGetPath)?;

        let paths = paths
            .get_value()
            .split(':')
            .map(Path::from_str)
            .collect::<Vec<&Path>>();

        let host = task_manager
            .get_environment_variable(task, "Host")
            .await
            .map_err(|_| Error::FailedToGetPath)?;
        self.host = host.get_value().to_string();

        if arguments.is_empty() {
            self.main_interactive(&paths).await?;
        } else {
            let arguments = arguments.iter().map(|s| s.as_str());
            self.parse_input(arguments, &paths).await?;
        }

        Ok(())
    }
}
