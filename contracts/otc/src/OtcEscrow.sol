// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @notice One fixed USDT obligation. Native EVER delivery is a separate chain leg.
contract OtcEscrow is ReentrancyGuard {
    using SafeERC20 for IERC20;

    enum State { Created, Funded, Released, Refunded, Expired }
    IERC20 public immutable token;
    address public immutable safe;
    address public immutable depositor;
    address public immutable recipient;
    bytes32 public immutable tradeId;
    bytes32 public immutable termsHash;
    uint256 public immutable usdtAmount;
    uint256 public immutable everAmount;
    uint256 public immutable createdAt;
    uint256 public immutable expiresAt;
    State public state;

    error Unauthorized();
    error Unavailable();
    error InexactDeposit();
    event Funded(bytes32 indexed tradeId, address indexed depositor, uint256 amount);
    event Released(bytes32 indexed tradeId, address indexed recipient, uint256 amount);
    event Refunded(bytes32 indexed tradeId, address indexed depositor, uint256 amount, bool expired);
    event Expired(bytes32 indexed tradeId);

    constructor(
        IERC20 token_, address safe_, address depositor_, address recipient_,
        bytes32 tradeId_, bytes32 termsHash_, uint256 usdtAmount_, uint256 everAmount_
    ) {
        token = token_;
        safe = safe_;
        depositor = depositor_;
        recipient = recipient_;
        tradeId = tradeId_;
        termsHash = termsHash_;
        usdtAmount = usdtAmount_;
        everAmount = everAmount_;
        createdAt = block.timestamp;
        expiresAt = block.timestamp + 4 hours;
    }

    /// @notice Pull the exact agreed deposit once; donations are not funding.
    function fund() external nonReentrant {
        if (msg.sender != depositor) revert Unauthorized();
        if (state != State.Created || block.timestamp >= expiresAt) revert Unavailable();
        uint256 beforeBalance = token.balanceOf(address(this));
        state = State.Funded;
        token.safeTransferFrom(depositor, address(this), usdtAmount);
        if (token.balanceOf(address(this)) - beforeBalance != usdtAmount) revert InexactDeposit();
        emit Funded(tradeId, depositor, usdtAmount);
    }

    /// @notice Only the immutable 2-of-3 Safe can release before expiry.
    function release() external nonReentrant {
        _beforeSettlement();
        state = State.Released;
        token.safeTransfer(recipient, usdtAmount);
        emit Released(tradeId, recipient, usdtAmount);
    }

    function refundEarly() external nonReentrant {
        _beforeSettlement();
        state = State.Refunded;
        token.safeTransfer(depositor, usdtAmount);
        emit Refunded(tradeId, depositor, usdtAmount, false);
    }

    /// @notice Anyone can call this after four hours, including during app outages.
    function refundExpired() external nonReentrant {
        if (block.timestamp < expiresAt) revert Unavailable();
        if (state == State.Created) {
            state = State.Expired;
            emit Expired(tradeId);
        } else if (state == State.Funded) {
            state = State.Refunded;
            token.safeTransfer(depositor, usdtAmount);
            emit Refunded(tradeId, depositor, usdtAmount, true);
        } else {
            revert Unavailable();
        }
    }

    /// @notice Unexpected ERC20 balances cannot change the obligation or beneficiary.
    function recoverSurplus(IERC20 asset) external nonReentrant {
        if (state != State.Released && state != State.Refunded && state != State.Expired) revert Unavailable();
        asset.safeTransfer(depositor, asset.balanceOf(address(this)));
    }

    function _beforeSettlement() private view {
        if (msg.sender != safe) revert Unauthorized();
        if (state != State.Funded || block.timestamp >= expiresAt) revert Unavailable();
    }
}
