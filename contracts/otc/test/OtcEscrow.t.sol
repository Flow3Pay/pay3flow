// SPDX-License-Identifier: AGPL-3.0-or-later
pragma solidity 0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {Safe} from "@safe-global/safe-contracts/contracts/Safe.sol";
import {Enum} from "@safe-global/safe-contracts/contracts/common/Enum.sol";
import {OtcEscrowFactory} from "../src/OtcEscrowFactory.sol";
import {OtcEscrow} from "../src/OtcEscrow.sol";
import {OtcEscrowGuard} from "../src/OtcEscrowGuard.sol";

interface Vm {
    function addr(uint256) external returns (address);
    function sign(uint256, bytes32) external returns (uint8, bytes32, bytes32);
    function prank(address) external;
    function warp(uint256) external;
    function expectRevert() external;
    function expectRevert(bytes4) external;
    function chainId(uint256) external;
    function etch(address, bytes calldata) external;
}

contract MockUsdt is ERC20 {
    bool public fail;
    bool public shortDeposit;
    constructor() ERC20("USDT", "USDT") {}
    function decimals() public pure override returns (uint8) { return 6; }
    function mint(address account, uint256 value) external { _mint(account, value); }
    function setFail(bool value) external { fail = value; }
    function setShort(bool value) external { shortDeposit = value; }
    function transfer(address to, uint256 value) public override returns (bool) {
        if (fail) return false;
        return super.transfer(to, value);
    }
    function transferFrom(address from, address to, uint256 value) public override returns (bool) {
        return super.transferFrom(from, to, shortDeposit ? value - 1 : value);
    }
}

contract OtcEscrowTest {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    MockUsdt internal token;
    OtcEscrowFactory internal factory;
    OtcEscrowFactory.Terms internal terms;
    Safe internal safe;
    OtcEscrow internal vault;
    address internal buyer;
    address internal seller;
    address internal arbitrator;

    function setUp() public {
        vm.warp(1_800_000_000);
        vm.chainId(1);
        buyer = vm.addr(11); seller = vm.addr(22); arbitrator = vm.addr(33);
        token = new MockUsdt();
        factory = new OtcEscrowFactory(token, arbitrator, address(new Safe()));
        terms = OtcEscrowFactory.Terms(keccak256("trade"), keccak256("terms"), buyer, seller, 100e6, 100_000e9, block.timestamp + 600);
        (address s, address v) = factory.createEscrow(terms, consent(11), consent(22));
        safe = Safe(payable(s)); vault = OtcEscrow(v);
        token.mint(buyer, 200e6);
    }

    function consent(uint256 key) internal returns (bytes memory) { return signature(key, factory.consentHash(terms)); }
    function signature(uint256 key, bytes32 digest) internal returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(key, digest);
        return abi.encodePacked(r, s, v);
    }
    function signatures(uint256 a, uint256 b, address to, bytes memory data, Enum.Operation op) internal returns (bytes memory) {
        bytes32 digest = safe.getTransactionHash(to, 0, data, op, 0, 0, 0, address(0), address(0), safe.nonce());
        bytes memory first = signature(a, digest); bytes memory second = signature(b, digest);
        return vm.addr(a) < vm.addr(b) ? bytes.concat(first, second) : bytes.concat(second, first);
    }
    function execute(uint256 a, uint256 b, bytes memory data) internal {
        bytes memory sigs = signatures(a, b, address(vault), data, Enum.Operation.Call);
        require(safe.execTransaction(address(vault), 0, data, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), sigs), "execution failed");
    }
    function executeFail(uint256 a, uint256 b, bytes memory data) internal {
        bytes memory sigs = signatures(a, b, address(vault), data, Enum.Operation.Call);
        vm.expectRevert();
        safe.execTransaction(address(vault), 0, data, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), sigs);
    }
    function fund() internal {
        vm.prank(buyer); token.approve(address(vault), terms.usdtAmount);
        vm.prank(buyer); vault.fund();
    }

    function testAllTwoOwnerPairsRelease() public {
        fund(); execute(11, 22, abi.encodeCall(OtcEscrow.release, ()));
        require(token.balanceOf(seller) == 100e6, "seller did not receive exact amount");
    }
    function testBuyerArbitratorRelease() public { fund(); execute(11, 33, abi.encodeCall(OtcEscrow.release, ())); }
    function testSellerArbitratorRelease() public { fund(); execute(22, 33, abi.encodeCall(OtcEscrow.release, ())); }
    function testEarlyRefund() public {
        fund(); execute(22, 33, abi.encodeCall(OtcEscrow.refundEarly, ()));
        require(token.balanceOf(buyer) == 200e6, "refund wrong amount or destination");
    }
    function testExpiryAnyoneRefundsAtExactBoundary() public {
        fund(); vm.warp(vault.expiresAt()); vm.prank(address(0x123)); vault.refundExpired();
        require(token.balanceOf(buyer) == 200e6, "timeout did not refund original depositor");
        vm.expectRevert(OtcEscrow.Unavailable.selector); vault.refundExpired();
    }
    function testRefundBeforeDeadlineFails() public {
        fund(); vm.warp(vault.expiresAt() - 1); vm.expectRevert(OtcEscrow.Unavailable.selector); vault.refundExpired();
    }
    function testUnfundedExpiresWithoutTransfer() public {
        vm.warp(vault.expiresAt()); vault.refundExpired();
        require(vault.state() == OtcEscrow.State.Expired, "unfunded expiry incorrect");
    }
    function testNoReleaseAtDeadlineEvenWithSignatures() public {
        fund(); vm.warp(vault.expiresAt()); executeFail(11, 22, abi.encodeCall(OtcEscrow.release, ()));
    }
    function testCannotFundAtDeadline() public {
        vm.prank(buyer); token.approve(address(vault), terms.usdtAmount);
        vm.warp(vault.expiresAt()); vm.prank(buyer); vm.expectRevert(OtcEscrow.Unavailable.selector); vault.fund();
    }
    function testOtherDepositorAndRepeatedFundingFail() public {
        vm.prank(seller); vm.expectRevert(OtcEscrow.Unauthorized.selector); vault.fund();
        fund(); vm.prank(buyer); vm.expectRevert(OtcEscrow.Unavailable.selector); vault.fund();
    }
    function testDonationDoesNotFundAndIsRecoveredOnlyAfterClosure() public {
        token.mint(address(vault), 7e6);
        require(vault.state() == OtcEscrow.State.Created, "donation credited as funding");
        vm.expectRevert(OtcEscrow.Unavailable.selector); vault.recoverSurplus(token);
        fund(); execute(11, 22, abi.encodeCall(OtcEscrow.release, ()));
        require(token.balanceOf(seller) == 100e6, "donation changed released amount");
        vault.recoverSurplus(token);
        require(token.balanceOf(buyer) == 107e6, "surplus destination incorrect");
    }
    function testInexactDepositRevertsWithoutFunding() public {
        token.setShort(true);
        vm.prank(buyer); token.approve(address(vault), 100e6);
        vm.prank(buyer); vm.expectRevert(OtcEscrow.InexactDeposit.selector); vault.fund();
        require(vault.state() == OtcEscrow.State.Created, "failed funding changed state");
    }
    function testTokenReleaseFailureCanStillRefundLater() public {
        fund(); token.setFail(true); executeFail(11, 22, abi.encodeCall(OtcEscrow.release, ()));
        require(vault.state() == OtcEscrow.State.Funded, "failed transfer closed escrow");
        token.setFail(false); vm.warp(vault.expiresAt()); vault.refundExpired();
    }
    function testReleaseThenRefundIsImpossible() public {
        fund(); execute(11, 22, abi.encodeCall(OtcEscrow.release, ()));
        vm.warp(vault.expiresAt()); vm.expectRevert(OtcEscrow.Unavailable.selector); vault.refundExpired();
    }
    function testOneSignatureCannotRelease() public {
        fund(); bytes memory data = abi.encodeCall(OtcEscrow.release, ());
        bytes32 digest = safe.getTransactionHash(address(vault), 0, data, Enum.Operation.Call, 0, 0, 0, address(0), address(0), 0);
        bytes memory sig = signature(11, digest);
        vm.expectRevert(); safe.execTransaction(address(vault), 0, data, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), sig);
    }
    function testDuplicateAndNonOwnerSignaturesFail() public {
        fund(); executeFail(11, 11, abi.encodeCall(OtcEscrow.release, ()));
        executeFail(11, 44, abi.encodeCall(OtcEscrow.release, ()));
    }
    function testGovernanceAndDelegateCallForbidden() public {
        bytes memory data = abi.encodeWithSignature("changeThreshold(uint256)", 1);
        bytes memory sigs = signatures(11, 22, address(safe), data, Enum.Operation.Call);
        vm.expectRevert(OtcEscrowGuard.ForbiddenTransaction.selector);
        safe.execTransaction(address(safe), 0, data, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), sigs);
        data = abi.encodeCall(OtcEscrow.release, ());
        sigs = signatures(11, 22, address(vault), data, Enum.Operation.DelegateCall);
        vm.expectRevert(OtcEscrowGuard.ForbiddenTransaction.selector);
        safe.execTransaction(address(vault), 0, data, Enum.Operation.DelegateCall, 0, 0, 0, address(0), payable(address(0)), sigs);
        (address[] memory modules,) = safe.getModulesPaginated(address(1), 10);
        require(modules.length == 0 && safe.getThreshold() == 2, "unsafe initialization");
    }
    function testCapPlusOneAndChangedConsentRejected() public {
        terms.tradeId = keccak256("another"); terms.everAmount = 100_000e9 + 1;
        bytes memory a = consent(11); bytes memory b = consent(22);
        vm.expectRevert(OtcEscrowFactory.InvalidTerms.selector); factory.createEscrow(terms, a, b);
        terms.everAmount = 1e9; a = consent(11); b = consent(22); terms.usdtAmount++;
        vm.expectRevert(OtcEscrowFactory.InvalidConsent.selector); factory.createEscrow(terms, a, b);
    }
    function testRepeatedTradeAndCrossChainConsentRejected() public {
        bytes memory a = consent(11); bytes memory b = consent(22);
        vm.expectRevert(OtcEscrowFactory.AlreadyCreated.selector); factory.createEscrow(terms, a, b);
        terms.tradeId = keccak256("cross-chain"); a = consent(11); b = consent(22); vm.chainId(2);
        vm.expectRevert(OtcEscrowFactory.InvalidConsent.selector); factory.createEscrow(terms, a, b);
    }
    function testExpiredConsentAndCollidingOwnersRejected() public {
        terms.tradeId = keccak256("expired-consent"); bytes memory a = consent(11); bytes memory b = consent(22);
        vm.warp(terms.validUntil + 1);
        vm.expectRevert(OtcEscrowFactory.InvalidTerms.selector); factory.createEscrow(terms, a, b);
        terms.validUntil = block.timestamp + 600; terms.seller = arbitrator; a = consent(11); b = consent(33);
        vm.expectRevert(OtcEscrowFactory.InvalidTerms.selector); factory.createEscrow(terms, a, b);
    }
    function testFuzzExpiryAlwaysReturnsPrincipal(uint32 elapsed) public {
        fund(); vm.warp(vault.expiresAt() + uint256(elapsed)); vault.refundExpired();
        require(token.balanceOf(buyer) == 200e6 && token.balanceOf(seller) == 0, "principal conservation broken");
    }
}
