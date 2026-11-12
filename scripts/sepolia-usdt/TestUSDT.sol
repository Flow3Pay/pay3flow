// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title TestUSDT - test-only ERC20 with 6 decimals (USDT-compatible interface)
/// @notice DEPLOY ON TESTNET ONLY. No access control, no audits, no value.
contract TestUSDT {
    string public name;
    string public symbol;
    uint8 public constant decimals = 6;

    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    constructor(string memory _name, string memory _symbol, uint256 _initialSupply, address _holder) {
        require(_holder != address(0), "holder is zero");
        name = _name;
        symbol = _symbol;
        totalSupply = _initialSupply;
        balanceOf[_holder] = _initialSupply;
        emit Transfer(address(0), _holder, _initialSupply);
    }

    function transfer(address to, uint256 value) external returns (bool) {
        _transfer(msg.sender, to, value);
        return true;
    }

    function approve(address spender, uint256 value) external returns (bool) {
        allowance[msg.sender][spender] = value;
        emit Approval(msg.sender, spender, value);
        return true;
    }

    function transferFrom(address from, address to, uint256 value) external returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed != type(uint256).max) {
            require(allowed >= value, "allowance exceeded");
            allowance[from][msg.sender] = allowed - value;
        }
        _transfer(from, to, value);
        return true;
    }

    /// @dev test helper: anyone can mint. Never use on a real network.
    function mint(address to, uint256 value) external {
        totalSupply += value;
        balanceOf[to] += value;
        emit Transfer(address(0), to, value);
    }

    function _transfer(address from, address to, uint256 value) internal {
        require(to != address(0), "transfer to zero");
        uint256 balance = balanceOf[from];
        require(balance >= value, "insufficient balance");
        unchecked {
            balanceOf[from] = balance - value;
            balanceOf[to] += value;
        }
        emit Transfer(from, to, value);
    }
}
