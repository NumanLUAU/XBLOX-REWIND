--[[
			// BaseScreen.lua

			// Creates a base screen with breadcrumbs and title. Do not use for a pane/tab
]]
local CoreGui = Game:GetService("CoreGui")
local GuiRoot = CoreGui:FindFirstChild("RobloxGui")
local Modules = GuiRoot:FindFirstChild("Modules")
local ShellModules = Modules:FindFirstChild("Shell")


local AssetManager = require(ShellModules:FindFirstChild('AssetManager'))
local GlobalSettings = require(ShellModules:FindFirstChild('GlobalSettings'))
local Utility = require(ShellModules:FindFirstChild('Utility'))

local function createBaseScreen(controller)
	local this = {}

	local container = Utility.Create'Frame'
	{
		Name = "Container";
		Size = UDim2.new(1, 0, 1, 0);
		BackgroundTransparency = 1;
	}
	local backImage = Utility.Create'ImageLabel'
	{
		Name = "BackImage";
		BackgroundTransparency = 1;
		Parent = container;
	}
	AssetManager.LocalImage(backImage,
		'rbxasset://textures/ui/Shell/Icons/BackIcon', {['720'] = UDim2.new(0,32,0,32); ['1080'] = UDim2.new(0,48,0,48);})
	local backText = Utility.Create'TextLabel'
	{
		Name = "BackText";
		Size = UDim2.new(0, 0, 0, backImage.Size.Y.Offset);
		Position = UDim2.new(0, backImage.Size.X.Offset + 8, 0, 0);
		BackgroundTransparency = 1;
		Font = GlobalSettings.RegularFont;
		FontSize = GlobalSettings.ButtonSize;
		TextXAlignment = Enum.TextXAlignment.Left;
		TextColor3 = GlobalSettings.WhiteTextColor;
		Text = "";
		Parent = container
	}
	local titleText = Utility.Create'TextLabel'
	{
		Name = "TitleText";
		Size = UDim2.new(0, 0, 0, 35);
		Position = UDim2.new(0, 16, 0, backImage.Size.Y.Offset + 74);
		BackgroundTransparency = 1;
		Font = GlobalSettings.LightFont;
		FontSize = GlobalSettings.HeaderSize;
		TextXAlignment = Enum.TextXAlignment.Left;
		TextColor3 = GlobalSettings.WhiteTextColor;
		Text = "";
		Parent = container;
	}

	--[[ Public API ]]--
	this.Container = container
	this.BackImage = backImage
	this.BackText = backText
	this.TitleText = titleText

	return this
end

return createBaseScreen
