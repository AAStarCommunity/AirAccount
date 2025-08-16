#!/usr/bin/env node
/**
 * AirAccount SDK Node.js Demo
 * 
 * Demonstrates how to use AirAccount SDK in a Node.js application
 * 
 * Usage: node demo.js [options]
 */

import { AirAccountSDK } from '../../src/index.js';
import readline from 'readline';
import chalk from 'chalk';

// Configuration
const DEFAULT_CA_URL = 'http://localhost:3001';

class AirAccountNodeDemo {
  constructor(caUrl = DEFAULT_CA_URL) {
    this.sdk = new AirAccountSDK({ caBaseUrl: caUrl });
    this.rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });
  }

  log(message, type = 'info') {
    const timestamp = new Date().toISOString();
    switch (type) {
      case 'success':
        console.log(chalk.green(`✅ [${timestamp}] ${message}`));
        break;
      case 'error':
        console.log(chalk.red(`❌ [${timestamp}] ${message}`));
        break;
      case 'warning':
        console.log(chalk.yellow(`⚠️  [${timestamp}] ${message}`));
        break;
      case 'info':
      default:
        console.log(chalk.blue(`ℹ️  [${timestamp}] ${message}`));
        break;
    }
  }

  async question(prompt) {
    return new Promise((resolve) => {
      this.rl.question(chalk.cyan(prompt), resolve);
    });
  }

  async initialize() {
    this.log('Initializing AirAccount SDK...');
    
    try {
      await this.sdk.initialize();
      this.log('SDK initialized successfully', 'success');
      return true;
    } catch (error) {
      this.log(`Failed to initialize SDK: ${error.message}`, 'error');
      this.log('Please ensure your CA service is running', 'warning');
      return false;
    }
  }

  async registerUser() {
    console.log(chalk.bold('\n🔐 User Registration with WebAuthn/Passkey\n'));
    
    const email = await this.question('Enter your email: ');
    const displayName = await this.question('Enter your display name: ');
    
    this.log(`Registering user: ${email}`);
    
    try {
      // Note: In real browser environment, this would use actual WebAuthn API
      // For Node.js demo, we simulate the process
      this.log('In a real browser, this would trigger WebAuthn/Passkey creation...', 'info');
      this.log('User would be prompted for biometric authentication (Face ID/Touch ID)', 'info');
      
      const result = await this.sdk.registerWithWebAuthn({
        email,
        displayName
      });
      
      if (result.success) {
        this.log(`Registration successful for ${email}`, 'success');
        this.log(`Credential ID: ${result.credentialId}`, 'info');
        return true;
      } else {
        this.log(`Registration failed: ${result.message}`, 'error');
        return false;
      }
    } catch (error) {
      this.log(`Registration error: ${error.message}`, 'error');
      return false;
    }
  }

  async authenticateUser() {
    console.log(chalk.bold('\n🔓 User Authentication with WebAuthn/Passkey\n'));
    
    const email = await this.question('Enter your email: ');
    
    this.log(`Authenticating user: ${email}`);
    
    try {
      this.log('In a real browser, this would verify your Passkey...', 'info');
      
      const result = await this.sdk.authenticateWithWebAuthn({ email });
      
      if (result.success) {
        this.log(`Authentication successful for ${email}`, 'success');
        return true;
      } else {
        this.log(`Authentication failed: ${result.message}`, 'error');
        return false;
      }
    } catch (error) {
      this.log(`Authentication error: ${error.message}`, 'error');
      return false;
    }
  }

  async createWallet() {
    console.log(chalk.bold('\n💰 Creating Wallet in TEE Hardware\n'));
    
    try {
      this.log('Creating wallet account in TEE...');
      const wallet = await this.sdk.createAccount();
      
      this.log('Wallet created successfully!', 'success');
      console.log(chalk.green('\nWallet Information:'));
      console.log(`  Wallet ID: ${wallet.id}`);
      console.log(`  Address: ${wallet.address}`);
      
      return wallet;
    } catch (error) {
      this.log(`Failed to create wallet: ${error.message}`, 'error');
      return null;
    }
  }

  async checkBalance() {
    console.log(chalk.bold('\n💰 Checking Wallet Balance\n'));
    
    try {
      this.log('Fetching balance from TEE...');
      const wallet = await this.sdk.getBalance();
      
      this.log('Balance retrieved successfully!', 'success');
      console.log(chalk.green('\nWallet Balance:'));
      console.log(`  Address: ${wallet.address}`);
      console.log(`  ETH Balance: ${wallet.balance?.eth || '0'} ETH`);
      
      if (wallet.balance?.tokens) {
        console.log('  Token Balances:');
        Object.entries(wallet.balance.tokens).forEach(([token, balance]) => {
          console.log(`    ${token}: ${balance}`);
        });
      }
      
      return wallet;
    } catch (error) {
      this.log(`Failed to get balance: ${error.message}`, 'error');
      return null;
    }
  }

  async sendTransfer() {
    console.log(chalk.bold('\n💸 Send Transfer\n'));
    
    const recipient = await this.question('Enter recipient address (0x...): ');
    const amount = await this.question('Enter amount (ETH): ');
    
    if (!recipient.startsWith('0x') || recipient.length !== 42) {
      this.log('Invalid recipient address format', 'error');
      return false;
    }
    
    const amountNum = parseFloat(amount);
    if (isNaN(amountNum) || amountNum <= 0) {
      this.log('Invalid amount', 'error');
      return false;
    }
    
    const confirm = await this.question(`Confirm transfer of ${amount} ETH to ${recipient}? (y/N): `);
    if (confirm.toLowerCase() !== 'y') {
      this.log('Transfer cancelled', 'info');
      return false;
    }
    
    try {
      this.log('Signing transaction in TEE hardware...');
      const result = await this.sdk.transfer({
        to: recipient,
        amount: amount
      });
      
      this.log('Transfer successful!', 'success');
      console.log(chalk.green('\nTransfer Details:'));
      console.log(`  Transaction Hash: ${result.txHash}`);
      console.log(`  Status: ${result.status}`);
      console.log(`  Amount: ${amount} ETH`);
      console.log(`  Recipient: ${recipient}`);
      
      if (result.gasUsed) {
        console.log(`  Gas Used: ${result.gasUsed}`);
      }
      
      return true;
    } catch (error) {
      this.log(`Transfer failed: ${error.message}`, 'error');
      return false;
    }
  }

  async listWallets() {
    console.log(chalk.bold('\n📋 List All Wallets\n'));
    
    try {
      this.log('Fetching wallet list...');
      const wallets = await this.sdk.listWallets();
      
      this.log(`Found ${wallets.length} wallet(s)`, 'success');
      
      if (wallets.length === 0) {
        console.log(chalk.yellow('  No wallets found. Create one first!'));
      } else {
        console.log(chalk.green('\nWallet List:'));
        wallets.forEach((wallet, index) => {
          console.log(`  ${index + 1}. Wallet ID: ${wallet.id}`);
          console.log(`     Address: ${wallet.address}`);
          console.log(`     Balance: ${wallet.balance?.eth || 'Unknown'} ETH`);
          console.log('');
        });
      }
      
      return wallets;
    } catch (error) {
      this.log(`Failed to list wallets: ${error.message}`, 'error');
      return [];
    }
  }

  async showMenu() {
    console.log(chalk.bold('\n📱 AirAccount Node.js Demo Menu\n'));
    
    const currentUser = this.sdk.getCurrentUser();
    if (currentUser) {
      console.log(chalk.green(`✅ Authenticated as: ${currentUser}\n`));
    } else {
      console.log(chalk.yellow('⚠️  Not authenticated\n'));
    }
    
    console.log('Available actions:');
    console.log('  1. Register new user');
    console.log('  2. Login existing user');
    console.log('  3. Create wallet');
    console.log('  4. Check balance');
    console.log('  5. Send transfer');
    console.log('  6. List wallets');
    console.log('  7. Logout');
    console.log('  8. Exit');
    console.log('');
    
    const choice = await this.question('Select an option (1-8): ');
    return choice.trim();
  }

  async run() {
    console.log(chalk.bold.blue('\n🛡️  AirAccount SDK Node.js Demo\n'));
    console.log('This demo shows how to use AirAccount SDK in Node.js applications');
    console.log('Note: WebAuthn features require a browser environment for full functionality\n');
    
    // Initialize SDK
    const initialized = await this.initialize();
    if (!initialized) {
      console.log(chalk.red('\nDemo cannot continue without CA service connection.'));
      console.log('Please start your CA service and try again:');
      console.log('  Rust CA: cargo run -p airaccount-ca-extended --bin ca-server');
      console.log('  Node.js CA: cd packages/airaccount-ca-nodejs && npm run dev');
      this.rl.close();
      return;
    }
    
    // Main menu loop
    let running = true;
    while (running) {
      try {
        const choice = await this.showMenu();
        
        switch (choice) {
          case '1':
            await this.registerUser();
            break;
          case '2':
            await this.authenticateUser();
            break;
          case '3':
            if (!this.sdk.isAuthenticated()) {
              this.log('Please login first', 'warning');
            } else {
              await this.createWallet();
            }
            break;
          case '4':
            if (!this.sdk.isAuthenticated()) {
              this.log('Please login first', 'warning');
            } else {
              await this.checkBalance();
            }
            break;
          case '5':
            if (!this.sdk.isAuthenticated()) {
              this.log('Please login first', 'warning');
            } else {
              await this.sendTransfer();
            }
            break;
          case '6':
            if (!this.sdk.isAuthenticated()) {
              this.log('Please login first', 'warning');
            } else {
              await this.listWallets();
            }
            break;
          case '7':
            this.sdk.logout();
            this.log('Logged out successfully', 'info');
            break;
          case '8':
            running = false;
            this.log('Goodbye!', 'info');
            break;
          default:
            this.log('Invalid option. Please try again.', 'warning');
        }
        
        if (running && choice !== '8') {
          await this.question('\nPress Enter to continue...');
        }
      } catch (error) {
        this.log(`Error: ${error.message}`, 'error');
        await this.question('\nPress Enter to continue...');
      }
    }
    
    this.rl.close();
  }
}

// Parse command line arguments
const args = process.argv.slice(2);
const caUrl = args.find(arg => arg.startsWith('--ca-url='))?.split('=')[1] || DEFAULT_CA_URL;

// Run demo
const demo = new AirAccountNodeDemo(caUrl);
demo.run().catch(error => {
  console.error(chalk.red('Demo failed:'), error);
  process.exit(1);
});

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
  console.log(chalk.yellow('\n\nDemo interrupted. Goodbye!'));
  process.exit(0);
});