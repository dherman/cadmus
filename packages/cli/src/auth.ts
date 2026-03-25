/**
 * Authentication commands: login flow and credential storage.
 */

import * as readline from 'node:readline';
import chalk from 'chalk';
import ora from 'ora';
import { CadmusClient, ApiError } from './api.js';
import { saveCredentials, loadCredentials, getCredentialsPath } from './config.js';

function question(rl: readline.Interface, prompt: string): Promise<string> {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => resolve(answer));
  });
}

function questionHidden(rl: readline.Interface, prompt: string): Promise<string> {
  return new Promise((resolve) => {
    process.stdout.write(prompt);

    // Mute stdout to hide password characters
    const origWrite = process.stdout.write.bind(process.stdout);
    process.stdout.write = ((_chunk: unknown) => {
      return true;
    }) as typeof process.stdout.write;

    rl.question('', (answer) => {
      process.stdout.write = origWrite;
      process.stdout.write('\n');
      resolve(answer);
    });
  });
}

export async function loginCommand(options: { server: string; token?: string }): Promise<void> {
  if (options.token) {
    saveCredentials({
      server: options.server,
      access_token: options.token,
      token_type: 'agent',
    });
    console.log(chalk.green('✓') + ' Authenticated with agent token');
    console.log(`  Credentials saved to ${getCredentialsPath()}`);
    return;
  }

  // Interactive login
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  try {
    const email = await question(rl, 'Email: ');
    const password = await questionHidden(rl, 'Password: ');
    rl.close();

    const spinner = ora('Authenticating...').start();
    try {
      const client = new CadmusClient(options.server, async () => '');
      const result = await client.login(email, password);
      spinner.succeed(`Authenticated as ${result.user.display_name} (${result.user.email})`);

      saveCredentials({
        server: options.server,
        access_token: result.access_token,
        refresh_token: result.refresh_token,
        token_type: 'jwt',
      });

      console.log(`  Credentials saved to ${getCredentialsPath()}`);
    } catch (err) {
      spinner.fail('Authentication failed');
      if (err instanceof ApiError && err.status === 401) {
        console.error(chalk.red('Error:') + ' Invalid email or password.');
      } else if (err instanceof Error) {
        console.error(chalk.red('Error:') + ` ${err.message}`);
      }
      return process.exit(1);
    }
  } catch {
    rl.close();
    throw new Error('Login cancelled');
  }
}

export async function statusCommand(): Promise<void> {
  const creds = loadCredentials();
  if (!creds) {
    console.log(chalk.yellow('Not logged in.') + " Run 'cadmus auth login' to authenticate.");
    return;
  }

  const spinner = ora('Checking credentials...').start();
  try {
    const client = new CadmusClient(creds.server, async () => creds.access_token);
    const user = await client.getMe();
    spinner.succeed(`Logged in as ${user.display_name} (${user.email})`);
    console.log(`  Server: ${creds.server}`);
    console.log(`  Token type: ${creds.token_type === 'agent' ? 'agent token' : 'JWT'}`);
  } catch (err) {
    spinner.fail('Credentials are invalid or expired');
    if (err instanceof ApiError && err.status === 401) {
      console.error(
        chalk.red('Error:') + " Session expired. Run 'cadmus auth login' to re-authenticate.",
      );
    } else if (err instanceof Error) {
      console.error(chalk.red('Error:') + ` ${err.message}`);
    }
    return process.exit(1);
  }
}
