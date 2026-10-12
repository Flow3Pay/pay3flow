use super::{Deployment, EscrowPolicy};
use crate::otc::{
    amount,
    error::{invalid, Error, Result},
    wallet,
};
use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::{eip712_domain, sol, SolCall, SolStruct};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use serde_json::{json, Value};

sol! {
    struct EscrowConsent {
        bytes32 tradeId;
        bytes32 termsHash;
        address buyer;
        address seller;
        address arbitrator;
        address token;
        uint256 usdtAmount;
        uint256 everAmount;
        uint256 validUntil;
    }
    struct SafeTx {
        address to;
        uint256 value;
        bytes data;
        uint8 operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }
    interface Factory {
        #[derive(Debug, PartialEq, Eq)]
        struct Terms {
            bytes32 tradeId; bytes32 termsHash; address buyer; address seller;
            uint256 usdtAmount; uint256 everAmount; uint256 validUntil;
        }
        function createEscrow(Terms terms, bytes buyerSignature, bytes sellerSignature) external;
        function token() external view returns (address);
        function arbitrator() external view returns (address);
        function singleton() external view returns (address);
        function vaults(bytes32 id) external view returns (address);
        event EscrowCreated(bytes32 indexed tradeId, address indexed safe, address indexed vault,
            bytes32 termsHash, uint256 createdAt, uint256 expiresAt);
    }
    interface Vault {
        function fund() external;
        function release() external;
        function refundEarly() external;
        function refundExpired() external;
        function recoverSurplus(address asset) external;
        function safe() external view returns (address);
        function state() external view returns (uint8);
        function expiresAt() external view returns (uint256);
        event Funded(bytes32 indexed tradeId, address indexed depositor, uint256 amount);
        event Released(bytes32 indexed tradeId, address indexed recipient, uint256 amount);
        event Refunded(bytes32 indexed tradeId, address indexed depositor, uint256 amount, bool expired);
        event Expired(bytes32 indexed tradeId);
    }
    interface Token {
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        event Transfer(address indexed from, address indexed to, uint256 amount);
    }
    interface Safe {
        function nonce() external view returns (uint256);
        function getOwners() external view returns (address[]);
        function getThreshold() external view returns (uint256);
        function execTransaction(address to, uint256 value, bytes data, uint8 operation,
            uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken,
            address refundReceiver, bytes signatures) external returns (bool);
        event ExecutionSuccess(bytes32 txHash, uint256 payment);
    }
}

pub(super) fn addr(value: &str) -> Result<Address> {
    wallet::address("ethereum", value)?
        .parse()
        .map_err(|_| invalid("invalid ABI address"))
}
pub(super) fn hash(value: &str) -> Result<B256> {
    value.parse().map_err(|_| invalid("invalid bytes32"))
}
pub(super) fn units(value: &str) -> Result<U256> {
    Ok(U256::from(amount::integer(value)?))
}
pub(super) fn hex(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", wallet::hex(bytes.as_ref()))
}
pub(super) fn onchain_terms(d: &Deployment) -> Result<Factory::Terms> {
    Ok(Factory::Terms {
        tradeId: hash(&d.trade_id)?,
        termsHash: hash(&d.terms_hash)?,
        buyer: addr(&d.buyer)?,
        seller: addr(&d.seller)?,
        usdtAmount: units(&d.usdt_amount)?,
        everAmount: units(&d.ever_amount)?,
        validUntil: U256::from(d.valid_until as u64),
    })
}
pub(super) fn consent(d: &Deployment, p: &EscrowPolicy) -> Result<(B256, Value)> {
    let t = onchain_terms(d)?;
    let message = EscrowConsent {
        tradeId: t.tradeId,
        termsHash: t.termsHash,
        buyer: t.buyer,
        seller: t.seller,
        arbitrator: addr(&p.arbitrator)?,
        token: addr(crate::otc::config::USDT)?,
        usdtAmount: t.usdtAmount,
        everAmount: t.everAmount,
        validUntil: t.validUntil,
    };
    let domain = eip712_domain! { name: "Pay3Flow OTC Escrow", version: "1", chain_id: 1, verifying_contract: addr(&p.factory)?, };
    let fields = json!([
        {"name":"tradeId","type":"bytes32"},{"name":"termsHash","type":"bytes32"},
        {"name":"buyer","type":"address"},{"name":"seller","type":"address"},
        {"name":"arbitrator","type":"address"},{"name":"token","type":"address"},
        {"name":"usdtAmount","type":"uint256"},{"name":"everAmount","type":"uint256"},
        {"name":"validUntil","type":"uint256"}
    ]);
    Ok((
        message.eip712_signing_hash(&domain),
        json!({"domain":{"name":"Pay3Flow OTC Escrow","version":"1","chainId":1,"verifyingContract":p.factory},"primaryType":"EscrowConsent","types":{"EscrowConsent":fields},"message":{"tradeId":d.trade_id,"termsHash":d.terms_hash,"buyer":d.buyer,"seller":d.seller,"arbitrator":p.arbitrator,"token":crate::otc::config::USDT,"usdtAmount":d.usdt_amount,"everAmount":d.ever_amount,"validUntil":d.valid_until.to_string()}}),
    ))
}

pub(super) fn safe_tx(safe: &str, vault: &str, action: &str, nonce: U256) -> Result<(B256, Value)> {
    let data = match action {
        "release" => Vault::releaseCall {}.abi_encode(),
        "refund_early" => Vault::refundEarlyCall {}.abi_encode(),
        _ => return Err(invalid("invalid Safe escrow action")),
    };
    let message = SafeTx {
        to: addr(vault)?,
        value: U256::ZERO,
        data: data.clone().into(),
        operation: 0,
        safeTxGas: U256::ZERO,
        baseGas: U256::ZERO,
        gasPrice: U256::ZERO,
        gasToken: Address::ZERO,
        refundReceiver: Address::ZERO,
        nonce,
    };
    let domain = eip712_domain! { chain_id: 1, verifying_contract: addr(safe)?, };
    let fields = json!([
        {"name":"to","type":"address"},{"name":"value","type":"uint256"},
        {"name":"data","type":"bytes"},{"name":"operation","type":"uint8"},
        {"name":"safeTxGas","type":"uint256"},{"name":"baseGas","type":"uint256"},
        {"name":"gasPrice","type":"uint256"},{"name":"gasToken","type":"address"},
        {"name":"refundReceiver","type":"address"},{"name":"nonce","type":"uint256"}
    ]);
    Ok((
        message.eip712_signing_hash(&domain),
        json!({"domain":{"chainId":1,"verifyingContract":safe},"primaryType":"SafeTx","types":{"SafeTx":fields},"message":{"to":vault,"value":"0","data":hex(data),"operation":0,"safeTxGas":"0","baseGas":"0","gasPrice":"0","gasToken":format!("{:#x}",Address::ZERO),"refundReceiver":format!("{:#x}",Address::ZERO),"nonce":nonce.to_string()}}),
    ))
}

/// Accept only canonical ECDSA typed-data signatures, also accepted by Safe.
pub(super) fn verify_signature(digest: B256, signature: &str, expected: &str) -> Result<String> {
    let mut bytes = wallet::unhex(signature)?;
    if bytes.len() != 65 {
        return Err(Error::Unauthorized);
    }
    let signature = Signature::from_slice(&bytes[..64]).map_err(|_| Error::Unauthorized)?;
    if signature.normalize_s().is_some() {
        return Err(Error::Unauthorized);
    }
    let v = match bytes[64] {
        27 | 28 => bytes[64] - 27,
        0 | 1 => bytes[64],
        _ => return Err(Error::Unauthorized),
    };
    let recovery = RecoveryId::try_from(v).map_err(|_| Error::Unauthorized)?;
    let key = VerifyingKey::recover_from_prehash(digest.as_slice(), &signature, recovery)
        .map_err(|_| Error::Unauthorized)?;
    let encoded = key.to_encoded_point(false);
    let actual = Address::from_slice(&alloy_primitives::keccak256(&encoded.as_bytes()[1..])[12..]);
    if actual != addr(expected)? {
        return Err(Error::Unauthorized);
    }
    bytes[64] = v + 27;
    Ok(hex(bytes))
}
