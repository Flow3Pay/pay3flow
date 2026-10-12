// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;

import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {EIP712} from "@openzeppelin/contracts/utils/cryptography/EIP712.sol";
import {Safe} from "@safe-global/safe-contracts/contracts/Safe.sol";
import {SafeProxy} from "@safe-global/safe-contracts/contracts/proxies/SafeProxy.sol";
import {OtcEscrow} from "./OtcEscrow.sol";
import {OtcEscrowGuard, OtcEscrowSetup} from "./OtcEscrowGuard.sol";

contract OtcEscrowFactory is EIP712 {
    uint256 public constant MAX_EVER = 100_000 * 1e9;
    uint256 public constant LIFETIME = 4 hours;
    bytes32 public constant CONSENT_TYPEHASH = keccak256(
        "EscrowConsent(bytes32 tradeId,bytes32 termsHash,address buyer,address seller,address arbitrator,address token,uint256 usdtAmount,uint256 everAmount,uint256 validUntil)"
    );
    struct Terms {
        bytes32 tradeId;
        bytes32 termsHash;
        address buyer;
        address seller;
        uint256 usdtAmount;
        uint256 everAmount;
        uint256 validUntil;
    }
    IERC20Metadata public immutable token;
    address public immutable arbitrator;
    address public immutable singleton;
    OtcEscrowSetup public immutable setupHelper;
    mapping(bytes32 => address) public vaults;

    error InvalidTerms();
    error AlreadyCreated();
    error InvalidConsent();
    event EscrowCreated(bytes32 indexed tradeId, address indexed safe, address indexed vault,
        bytes32 termsHash, uint256 createdAt, uint256 expiresAt);

    constructor(IERC20Metadata token_, address arbitrator_, address singleton_)
        EIP712("Pay3Flow OTC Escrow", "1")
    {
        if (address(token_).code.length == 0 || token_.decimals() != 6
            || arbitrator_ == address(0) || arbitrator_.code.length != 0
            || singleton_.code.length == 0
            || keccak256(bytes(Safe(payable(singleton_)).VERSION())) != keccak256("1.4.1")) revert InvalidTerms();
        token = token_;
        arbitrator = arbitrator_;
        singleton = singleton_;
        setupHelper = new OtcEscrowSetup();
    }

    function consentHash(Terms calldata terms) public view returns (bytes32) {
        return _hashTypedDataV4(keccak256(abi.encode(CONSENT_TYPEHASH,
            terms.tradeId, terms.termsHash, terms.buyer, terms.seller, arbitrator, address(token),
            terms.usdtAmount, terms.everAmount, terms.validUntil)));
    }

    /// @notice Both traders consent; callers cannot substitute terms or seize a trade ID.
    function createEscrow(Terms calldata terms, bytes calldata buyerSignature, bytes calldata sellerSignature)
        external returns (address safeAddress, address vaultAddress)
    {
        if (terms.tradeId == bytes32(0) || terms.termsHash == bytes32(0)
            || terms.buyer == address(0) || terms.seller == address(0) || terms.buyer == terms.seller
            || terms.buyer == arbitrator || terms.seller == arbitrator
            || terms.buyer.code.length != 0 || terms.seller.code.length != 0 || arbitrator.code.length != 0
            || terms.usdtAmount == 0 || terms.everAmount == 0 || terms.everAmount > MAX_EVER
            || block.timestamp > terms.validUntil) revert InvalidTerms();
        if (vaults[terms.tradeId] != address(0)) revert AlreadyCreated();
        bytes32 digest = consentHash(terms);
        if (ECDSA.recover(digest, buyerSignature) != terms.buyer
            || ECDSA.recover(digest, sellerSignature) != terms.seller) revert InvalidConsent();

        // Factory-scoped CREATE2 prevents outsiders from predeploying a proxy to block this trade.
        // Initialization completes atomically; an uninitialized Safe cannot escape this transaction.
        safeAddress = address(new SafeProxy{salt: terms.tradeId}(singleton));
        OtcEscrow vault = new OtcEscrow(token, safeAddress, terms.buyer, terms.seller,
            terms.tradeId, terms.termsHash, terms.usdtAmount, terms.everAmount);
        vaultAddress = address(vault);
        OtcEscrowGuard guard = new OtcEscrowGuard(safeAddress, vaultAddress);
        address[] memory owners = new address[](3);
        owners[0] = terms.buyer; owners[1] = terms.seller; owners[2] = arbitrator;
        Safe(payable(safeAddress)).setup(owners, 2, address(setupHelper),
            abi.encodeCall(OtcEscrowSetup.install, (address(guard))), address(0), address(0), 0, payable(address(0)));
        vaults[terms.tradeId] = vaultAddress;
        emit EscrowCreated(terms.tradeId, safeAddress, vaultAddress, terms.termsHash, block.timestamp, vault.expiresAt());
    }
}
