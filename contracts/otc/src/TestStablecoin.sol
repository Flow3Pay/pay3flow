// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @notice Faucet token for isolated OTC testnets. Never deploy on a production chain.
contract TestStablecoin is ERC20 {
    uint8 private immutable tokenDecimals;

    constructor(
        string memory tokenName,
        string memory tokenSymbol,
        uint8 decimals_,
        address holder,
        uint256 initialSupply
    ) ERC20(tokenName, tokenSymbol) {
        require(holder != address(0), "holder is zero");
        tokenDecimals = decimals_;
        _mint(holder, initialSupply);
    }

    function decimals() public view override returns (uint8) {
        return tokenDecimals;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}
