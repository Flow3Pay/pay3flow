module test_tokens::usdc {
    use std::signer;
    use std::string;
    use aptos_framework::coin::{
        Self, BurnCapability, FreezeCapability, MintCapability,
    };

    struct USDC has store {}

    struct Capabilities has key {
        burn: BurnCapability<USDC>,
        freeze: FreezeCapability<USDC>,
        mint: MintCapability<USDC>,
    }

    fun init_module(account: &signer) {
        let (burn, freeze, mint) = coin::initialize<USDC>(
            account,
            string::utf8(b"Pay3Flow Test USDC"),
            string::utf8(b"USDC"),
            6,
            true,
        );
        coin::register<USDC>(account);
        coin::deposit(
            signer::address_of(account),
            coin::mint<USDC>(1_000_000_000_000, &mint),
        );
        move_to(account, Capabilities { burn, freeze, mint });
    }

    public entry fun mint(account: &signer, amount: u64) acquires Capabilities {
        let capabilities = borrow_global<Capabilities>(@test_tokens);
        coin::register<USDC>(account);
        coin::deposit(
            signer::address_of(account),
            coin::mint<USDC>(amount, &capabilities.mint),
        );
    }
}
