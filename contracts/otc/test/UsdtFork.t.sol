// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;
import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {Safe} from "@safe-global/safe-contracts/contracts/Safe.sol";
import {OtcEscrowFactory} from "../src/OtcEscrowFactory.sol";
import {OtcEscrow} from "../src/OtcEscrow.sol";

interface ForkVm {
    function addr(uint256) external returns (address);
    function sign(uint256,bytes32) external returns (uint8,bytes32,bytes32);
    function prank(address) external;
    function warp(uint256) external;
    function record() external;
    function accesses(address) external returns (bytes32[] memory,bytes32[] memory);
    function store(address,bytes32,bytes32) external;
    function load(address,bytes32) external returns (bytes32);
}

/// @notice Opt-in test against real USDT runtime/storage on a LOCAL mainnet fork.
contract UsdtForkTest {
    ForkVm private constant vm=ForkVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    uint256 private constant BUYER_KEY=uint256(keccak256("Pay3Flow fork-only buyer"));
    uint256 private constant SELLER_KEY=uint256(keccak256("Pay3Flow fork-only seller"));
    uint256 private constant ARBITRATOR_KEY=uint256(keccak256("Pay3Flow fork-only arbitrator"));
    IERC20Metadata private constant usdt=IERC20Metadata(0xdAC17F958D2ee523a2206206994597C13D831ec7);
    function signature(uint256 key,bytes32 digest) private returns(bytes memory) {
        (uint8 v,bytes32 r,bytes32 s)=vm.sign(key,digest);return abi.encodePacked(r,s,v);
    }
    function fundForkBalance(address buyer) private {
        vm.record();usdt.balanceOf(buyer);
        (bytes32[] memory reads,)=vm.accesses(address(usdt));
        for(uint256 i=0;i<reads.length;i++) {
            // Ignore scalar slots (including USDT's deprecation flag); probe only mapping slots.
            if(uint256(reads[i])<type(uint128).max) continue;
            bytes32 original=vm.load(address(usdt),reads[i]);
            vm.store(address(usdt),reads[i],bytes32(uint256(100e6)));
            if(usdt.balanceOf(buyer)==100e6) return;
            vm.store(address(usdt),reads[i],original);
        }
        revert("fork balance setup failed");
    }
    function testActualUsdtFundingAndPermissionlessRefund() public {
        // Ordinary offline test runs have no USDT deployment and skip this fork-only case.
        if (address(usdt).code.length==0) return;
        require(block.chainid==1,"mainnet fork required");
        address buyer=vm.addr(BUYER_KEY);address seller=vm.addr(SELLER_KEY);
        // Only mutate local fork storage; no transaction touches Ethereum mainnet.
        fundForkBalance(buyer);
        OtcEscrowFactory factory=new OtcEscrowFactory(usdt,vm.addr(ARBITRATOR_KEY),address(new Safe()));
        OtcEscrowFactory.Terms memory terms=OtcEscrowFactory.Terms(keccak256("fork-trade"),keccak256("fork-terms"),buyer,seller,100e6,100_000e9,block.timestamp+600);
        bytes32 digest=factory.consentHash(terms);
        (,address vaultAddress)=factory.createEscrow(terms,signature(BUYER_KEY,digest),signature(SELLER_KEY,digest));
        // USDT's real approval and transfers return no boolean. Low-level approve is intentional.
        vm.prank(buyer);(bool approved,)=address(usdt).call(abi.encodeWithSignature("approve(address,uint256)",vaultAddress,100e6));
        require(approved,"USDT approval failed");
        OtcEscrow vault=OtcEscrow(vaultAddress);vm.prank(buyer);vault.fund();
        require(usdt.balanceOf(vaultAddress)==100e6,"actual USDT deposit failed");
        vm.warp(vault.expiresAt());vm.prank(address(0x123));vault.refundExpired();
        require(usdt.balanceOf(buyer)==100e6,"actual USDT refund failed");
    }
}
