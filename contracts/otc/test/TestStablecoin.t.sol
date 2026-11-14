// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;

import {TestStablecoin} from "../src/TestStablecoin.sol";

contract TestStablecoinTest {
    function testFaucetTokenHasConfiguredPrecisionAndCanMint() public {
        address holder = address(0x1234);
        TestStablecoin usdt = new TestStablecoin("Test Tether", "USDT", 6, holder, 100e6);
        TestStablecoin usdc = new TestStablecoin("Test USD Coin", "USDC", 6, holder, 200e6);

        require(usdt.decimals() == 6 && usdc.decimals() == 6, "wrong precision");
        require(usdt.balanceOf(holder) == 100e6, "initial USDT missing");
        require(usdc.balanceOf(holder) == 200e6, "initial USDC missing");
        usdt.mint(address(this), 5e6);
        require(usdt.balanceOf(address(this)) == 5e6, "USDT faucet mint failed");
    }
}
