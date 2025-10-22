//! Test harness for E2E tests with solana-test-validator

use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

/// Test validator process handle
pub struct TestValidator {
    _process: Child,
    rpc_url: String,
}

impl TestValidator {
    /// Start a new test validator
    pub fn start() -> Result<Self> {
        println!("Starting solana-test-validator...");

        let process = Command::new("solana-test-validator")
            .arg("--reset")
            .arg("--quiet")
            .spawn()
            .context("Failed to start solana-test-validator")?;

        // Wait for validator to start
        thread::sleep(Duration::from_secs(3));

        Ok(Self {
            _process: process,
            rpc_url: "http://localhost:8899".to_string(),
        })
    }

    /// Get RPC client
    pub fn rpc_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(&self.rpc_url, CommitmentConfig::confirmed())
    }
}

impl Drop for TestValidator {
    fn drop(&mut self) {
        println!("Stopping test validator...");
        let _ = Command::new("solana-test-validator").arg("exit").output();
    }
}

/// Test context with deployed programs
pub struct TestContext {
    pub validator: TestValidator,
    pub client: RpcClient,
    pub payer: Keypair,
    pub slab_program_id: Pubkey,
    pub router_program_id: Pubkey,
    pub oracle_program_id: Pubkey,
}

impl TestContext {
    /// Initialize test context with deployed programs
    pub async fn new() -> Result<Self> {
        let validator = TestValidator::start()?;
        let client = validator.rpc_client();

        // Request airdrop for payer
        let payer = Keypair::new();
        println!("Requesting airdrop for payer: {}", payer.pubkey());

        let _signature = client
            .request_airdrop(&payer.pubkey(), 10_000_000_000)
            .context("Failed to request airdrop")?;

        // Wait for airdrop
        for _ in 0..30 {
            if let Ok(balance) = client.get_balance(&payer.pubkey()) {
                if balance > 0 {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(500));
        }

        println!("Payer balance: {} SOL", client.get_balance(&payer.pubkey())? as f64 / 1e9);

        // Deploy programs
        println!("Deploying programs...");
        let slab_program_id = Self::deploy_program(&client, &payer, "target/deploy/percolator_slab.so")?;
        let router_program_id = Self::deploy_program(&client, &payer, "target/deploy/percolator_router.so")?;
        let oracle_program_id = Self::deploy_program(&client, &payer, "target/deploy/percolator_oracle.so")?;

        println!("Programs deployed:");
        println!("  Slab:   {}", slab_program_id);
        println!("  Router: {}", router_program_id);
        println!("  Oracle: {}", oracle_program_id);

        Ok(Self {
            validator,
            client,
            payer,
            slab_program_id,
            router_program_id,
            oracle_program_id,
        })
    }

    /// Deploy a program from .so file
    fn deploy_program(_client: &RpcClient, _payer: &Keypair, program_path: &str) -> Result<Pubkey> {
        println!("Deploying {}...", program_path);

        // Use solana program deploy command
        let output = Command::new("solana")
            .args(&["program", "deploy", "--url", "http://localhost:8899", program_path])
            .output()
            .context("Failed to deploy program")?;

        if !output.status.success() {
            anyhow::bail!(
                "Program deployment failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Parse program ID from output
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("Program Id:") {
                if let Some(id_str) = line.split_whitespace().last() {
                    return id_str.parse().context("Failed to parse program ID");
                }
            }
        }

        anyhow::bail!("Failed to find program ID in deployment output")
    }

    /// Create and fund a new account
    pub fn create_account(
        &self,
        size: usize,
        owner: &Pubkey,
    ) -> Result<Keypair> {
        let account = Keypair::new();
        let rent = self.client.get_minimum_balance_for_rent_exemption(size)?;

        let instruction = solana_sdk::system_instruction::create_account(
            &self.payer.pubkey(),
            &account.pubkey(),
            rent,
            size as u64,
            owner,
        );

        let recent_blockhash = self.client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer, &account],
            recent_blockhash,
        );

        self.client.send_and_confirm_transaction(&transaction)?;

        Ok(account)
    }

    /// Get account data
    pub fn get_account_data(&self, pubkey: &Pubkey) -> Result<Vec<u8>> {
        let account = self
            .client
            .get_account(pubkey)
            .context("Failed to get account")?;
        Ok(account.data)
    }
}
