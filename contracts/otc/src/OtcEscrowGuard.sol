// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;

import {BaseGuard} from "@safe-global/safe-contracts/contracts/base/GuardManager.sol";
import {Enum} from "@safe-global/safe-contracts/contracts/common/Enum.sol";
import {Safe} from "@safe-global/safe-contracts/contracts/Safe.sol";
import {OtcEscrow} from "./OtcEscrow.sol";

/// @notice A trade Safe can only release/refund its vault; its governance is frozen.
contract OtcEscrowGuard is BaseGuard {
    address public immutable safe;
    address public immutable vault;
    error ForbiddenTransaction();

    constructor(address safe_, address vault_) { safe = safe_; vault = vault_; }

    function checkTransaction(
        address to, uint256 value, bytes memory data, Enum.Operation operation,
        uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken,
        address payable refundReceiver, bytes memory, address
    ) external view override {
        if (msg.sender != safe || to != vault || value != 0 || operation != Enum.Operation.Call
            || safeTxGas != 0 || baseGas != 0 || gasPrice != 0 || gasToken != address(0)
            || refundReceiver != address(0) || data.length != 4) revert ForbiddenTransaction();
        bytes4 selector = bytes4(data);
        if (selector != OtcEscrow.release.selector && selector != OtcEscrow.refundEarly.selector) revert ForbiddenTransaction();
    }

    function checkAfterExecution(bytes32, bool success) external view override {
        if (msg.sender != safe || !success) revert ForbiddenTransaction();
    }
}

/// @notice Only used as a delegatecall during the factory's atomic Safe setup.
contract OtcEscrowSetup {
    address private immutable self = address(this);
    error OnlyDelegateCall();
    function install(address guard) external {
        if (address(this) == self) revert OnlyDelegateCall();
        // External self-call meets Safe's authorized-only self-call requirement.
        Safe(payable(address(this))).setGuard(guard);
    }
}
