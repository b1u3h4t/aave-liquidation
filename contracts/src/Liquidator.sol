// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "@forge-std/console.sol";

import {IERC20} from "@openzeppelin/token/ERC20/IERC20.sol";
import {Ownable} from "@openzeppelin/access/Ownable.sol";
import {IFlashLoanSimpleReceiver} from "@aave-v3/misc/flashloan/interfaces/IFlashLoanSimpleReceiver.sol";
import {FlashLoanSimpleReceiverBase} from "@aave-v3/misc/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import {IPoolAddressesProvider} from "@aave-v3/interfaces/IPoolAddressesProvider.sol";
import {ISwapRouter} from "@uniswap-v3-periphery/interfaces/ISwapRouter.sol";

contract Liquidatoor is FlashLoanSimpleReceiverBase, Ownable {
    ISwapRouter public immutable swapRouter;

    receive() external payable {}

    constructor(
        IPoolAddressesProvider _addressProvider,
        ISwapRouter _swapRouter
    ) FlashLoanSimpleReceiverBase(_addressProvider) Ownable(msg.sender) {
        swapRouter = _swapRouter;
    }

    /*
     * @notice Withdraws ETH from the contract.
     */
    function withdrawalETH() external onlyOwner {
        (bool success, ) = payable(msg.sender).call{
            value: address(this).balance
        }("");
        require(success, "ETH transfer failed");
    }

    /*
     * @notice Withdraws ERC20 tokens from the contract.
     */
    function withdrawalERC20(address token) external onlyOwner {
        IERC20(token).transfer(
            msg.sender,
            IERC20(token).balanceOf(address(this))
        );
    }

    /*
     * @notice Called by the Aave Pool after your contract has received the flashloan.
     */
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        (address collateral_asset, address user, uint24 fee) = abi.decode(
            params,
            (address, address, uint24)
        );

        IERC20(asset).approve(address(POOL), amount);
        uint256 collateralBalance = IERC20(collateral_asset).balanceOf(
            address(this)
        );

        POOL.liquidationCall(
            collateral_asset,
            asset, // debt asset
            user,
            amount,
            false
        );

        if (collateral_asset != asset) {
            collateralBalance = IERC20(collateral_asset).balanceOf(
                address(this)
            );

            // approve the Uniswap router to spend the collateral.
            IERC20(collateral_asset).approve(
                address(swapRouter),
                collateralBalance
            );
            IERC20(collateral_asset).allowance(
                address(this),
                address(swapRouter)
            );

            // setup swap parameters
            ISwapRouter.ExactInputSingleParams memory swapParams = ISwapRouter
                .ExactInputSingleParams({
                    tokenIn: collateral_asset, // ETH
                    tokenOut: asset, // USDT
                    fee: fee,
                    recipient: address(this),
                    deadline: block.timestamp + 10,
                    amountIn: collateralBalance,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                });

            swapRouter.exactInputSingle(swapParams);
        }

        uint256 totalRepayment = amount + premium;
        // Approve the pool to pull the flash loan repayment
        IERC20(asset).approve(address(POOL), totalRepayment);

        return true;
    }

    /*
     * @notice Initiates a flashloan to liquidate an undercollateralized position.
     * @param asset The debt asset to be repaid through the flashloan.
     * @param collateral The collateral asset to be liquidated.
     * @param userToLiquidate The address of the user to liquidate.
     * @param amount The amount to liquidate.
     */
    function liquidatoor(
        address debt_asset,
        address collateral_asset,
        address userToLiquidate,
        uint256 amount,
        uint24 fee
    ) external onlyOwner {
        // pack collateral and user for use in executeOperation
        bytes memory params = abi.encode(
            collateral_asset,
            userToLiquidate,
            fee
        );

        // Initiate the flashLoan with the updated parameters
        POOL.flashLoanSimple(
            address(this),
            debt_asset,
            amount,
            params,
            0
        );
    }
}
