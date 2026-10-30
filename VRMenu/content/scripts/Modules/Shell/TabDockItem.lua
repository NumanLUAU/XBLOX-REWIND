-- Written by Kip Turner, Copyright ROBLOX 2015

local TextService = game:GetService('TextService')
local UserInputService = game:GetService('UserInputService')

local CoreGui = game:GetService("CoreGui")
local GuiRoot = CoreGui:FindFirstChild("RobloxGui")
local Modules = GuiRoot:FindFirstChild("Modules")
local ShellModules = Modules:FindFirstChild("Shell")

local Utility = require(ShellModules:FindFirstChild('Utility'))
local GlobalSettings = require(ShellModules:FindFirstChild('GlobalSettings'))
local SoundManager = require(ShellModules:FindFirstChild('SoundManager'))

local function CreateTabDockItem(tabName, contentItem)
	local this = {}
	local name = tabName
	local selected = false
	local focused = false
	local content = contentItem

	this.SizeChanged = Utility.Signal()
	this.Clicked = Utility.Signal()

	local tabItem;
	if Utility.ShouldUseVRAppLobby() then
		tabItem = Utility.Create'TextButton'
		{
			Text = name;
			Size = UDim2.new(0, 100, 1, 0);
			BackgroundTransparency = 1;
			Name = 'TabItem';
			FontSize = GlobalSettings.HeaderSize;
			Font = GlobalSettings.LightFont;
			BackgroundTransparency = 1;
			Selectable = UserInputService.VREnabled;
		}

		tabItem.MouseButton1Down:connect(function()
			this:OnClick()
		end)
		tabItem.MouseButton1Up:connect(function()
			this:OnClickRelease()
			this.Clicked:fire()
		end)
	else
		tabItem = Utility.Create'TextLabel'
		{
			Text = name;
			Size = UDim2.new(0, 100, 1, 0);
			BackgroundTransparency = 1;
			Name = 'TabItem';
			FontSize = GlobalSettings.HeaderSize;
			Font = GlobalSettings.LightFont;
			BackgroundTransparency = 1;
		}
	end
	do
		local tabItemTextSize = TextService:GetTextSize(tabItem.Text, Utility.ConvertFontSizeEnumToInt(tabItem.FontSize), tabItem.Font, Vector2.new())
		tabItem.Size = UDim2.new(0,tabItemTextSize.X,1,0)
		this.SizeChanged:fire(tabItem.Size)
	end
	local smallText = Utility.Create'TextLabel'
	{
		Text = name;
		Size = UDim2.new(0, 0, 0, 0);
		Position = UDim2.new(0.5, 0, 0.5, 0);
		BackgroundTransparency = 1;
		Name = 'SmallText';
		FontSize = GlobalSettings.MediumFontSize;
		Font = GlobalSettings.LightFont;
		TextColor3 = GlobalSettings.WhiteTextColor;
		BackgroundTransparency = 1;
		Visible = false;
		Parent = tabItem;
	}

	local function OnSelectionChanged()
		if selected then
			tabItem.TextColor3 = GlobalSettings.WhiteTextColor;
		else
			tabItem.TextColor3 = GlobalSettings.BlueTextColor;
			if Utility.ShouldUseVRAppLobby() then
				this:SetFocused(false)
			end
		end
	end

	function this:GetContentItem()
		return content
	end

	function this:GetGuiObject()
		return tabItem
	end

	function this:SetSelected(isSelected)
		if selected ~= isSelected then
			selected = isSelected
			OnSelectionChanged()
		end
	end

	function this:GetSelected()
		return selected
	end

	function this:GetName()
		return name
	end

	function this:GetSize()
		return tabItem.Size
	end

	function this:OnClick()
		smallText.Visible = true;
		tabItem.TextTransparency = 1;
		SoundManager:Play('ButtonPress')
	end

	function this:OnClickRelease()
		smallText.Visible = false;
		tabItem.TextTransparency = 0;
	end

	function this:SetFocused(nowFocused)
		if focused ~= nowFocused then
			focused = nowFocused

			if focused then
				for _, inputObject in pairs(UserInputService:GetGamepadState(Enum.UserInputType.Gamepad1)) do
					if inputObject.KeyCode == Enum.KeyCode.ButtonA and
							inputObject.UserInputState == Enum.UserInputState.Begin then
						self:OnClick()
					end
				end
			else
				self:OnClickRelease()
			end

		end
	end

	function this:SetPosition(newPosition)
		tabItem.Position = newPosition
	end

	function this:SetParent(newParent)
		tabItem.Parent = newParent
	end
	-- Initialize
	OnSelectionChanged()

	return this
end


return CreateTabDockItem
