// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;
import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {OtcEscrowFactory} from "../src/OtcEscrowFactory.sol";

interface DeployVm {
    function envAddress(string calldata) external returns (address);
    function startBroadcast() external;
    function stopBroadcast() external;
}

/// @notice Broadcast only with an explicitly selected local keystore and mainnet RPC.
contract Deploy {
    DeployVm private constant vm = DeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    event Deployment(address factory, bytes32 factoryCodeHash, address singleton, bytes32 singletonCodeHash, address arbitrator);
    function run() external returns (OtcEscrowFactory factory) {
        require(block.chainid == 1, "Ethereum mainnet chain ID required");
        address arbitrator = vm.envAddress("OTC_ARBITRATOR");
        address singleton = vm.envAddress("SAFE_SINGLETON");
        vm.startBroadcast();
        factory = new OtcEscrowFactory(IERC20Metadata(0xdAC17F958D2ee523a2206206994597C13D831ec7), arbitrator, singleton);
        vm.stopBroadcast();
        emit Deployment(address(factory), address(factory).codehash, singleton, singleton.codehash, arbitrator);
    }
}
