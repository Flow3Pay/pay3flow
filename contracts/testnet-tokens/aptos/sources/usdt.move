module test_tokens::usdt {
    use std::signer;
    use std::string;
    use aptos_framework::coin::{
        Self, BurnCapability, FreezeCapability, MintCapability,
    };

    struct USDT has store {}

    struct Capabilities has key {
        burn: BurnCapability<USDT>,
        freeze: FreezeCapability<USDT>,
        mint: MintCapability<USDT>,
    }

    fun init_module(account: &signer) {
        let (burn, freeze, mint) = coin::initialize<USDT>(
            account,
            string::utf8(b"Pay3Flow Test USDT"),
            string::utf8(b"USDT"),
            6,
            true,
        );
        coin::register<USDT>(account);
        coin::deposit(
            signer::address_of(account),
            coin::mint<USDT>(1_000_000_000_000, &mint),
        );
        move_to(account, Capabilities { burn, freeze, mint });
    }

    public entry fun mint(account: &signer, amount: u64) acquires Capabilities {
        let capabilities = borrow_global<Capabilities>(@test_tokens);
        coin::register<USDT>(account);
        coin::deposit(
            signer::address_of(account),
            coin::mint<USDT>(amount, &capabilities.mint),
        );
    }
}
