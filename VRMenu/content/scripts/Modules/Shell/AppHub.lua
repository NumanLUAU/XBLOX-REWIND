-- Written by Kip Turner, Copyright ROBLOX 2015

local CoreGui = game:GetService("CoreGui")
local ContextActionService = game:GetService("ContextActionService")
local PlatformService = nil
pcall(function() PlatformService = game:GetService('PlatformService') end)
local UserInputService = game:GetService("UserInputService")
local GuiService = game:GetService('GuiService')
local TextService = game:GetService('TextService')

local GuiRoot = CoreGui:FindFirstChild("RobloxGui")
local Modules = GuiRoot:FindFirstChild("Modules")
local ShellModules = Modules:FindFirstChild("Shell")

local Utility = require(ShellModules:FindFirstChild('Utility'))
local GlobalSettings = require(ShellModules:FindFirstChild('GlobalSettings'))
local AppTabDockModule = require(ShellModules:FindFirstChild('TabDock'))
local AppTabDockItemModule = require(ShellModules:FindFirstChild('TabDockItem'))
local HomePaneModule = require(ShellModules:FindFirstChild('HomePane'))
local GamePaneModule = require(ShellModules:FindFirstChild('GamePane'))
local AvatarPaneModule = require(ShellModules:FindFirstChild('AvatarPane'))
local Errors = require(ShellModules:FindFirstChild('Errors'))
local ErrorOverlay = require(ShellModules:FindFirstChild('ErrorOverlay'))
local EventHub = require(ShellModules:FindFirstChild('EventHub'))
local ScreenManager = require(ShellModules:FindFirstChild('ScreenManager'))
local SocialPaneModule = nil
if Utility.IsFastFlagEnabled("XboxFriendsEvents") then
	SocialPaneModule = require(ShellModules:FindFirstChild('FriendsPane'))
else
	SocialPaneModule = require(ShellModules:FindFirstChild('SocialPane'))
end
local StorePaneModule = require(ShellModules:FindFirstChild('StorePane'))
local Strings = require(ShellModules:FindFirstChild('LocalizedStrings'))
local SettingsScreen = require(ShellModules:FindFirstChild('SettingsScreen'))
local GameSearchScreen = require(ShellModules:FindFirstChild('GameSearchScreen'))

local XboxGameSearchEnabled = Utility.IsFastFlagEnabled('XboxEnableGameSearch')

local function CreateAppHub()
	local this = {}

	local AppTabDock = AppTabDockModule()
	local appHubCns = {}
	local VRChangedConnection = nil
	local SelectedTabChangedConnection = nil

	local isShown = false
	local isFocused = false

	-- TODO: Remove this when we remove the Utility.ShouldUseVRAppLobby() checks
	local lastSelectedContentPane = nil
	local lastParent = nil

	local HubContainer = Utility.Create'Frame'
	{
		Name = 'HubContainer';
		Size = UDim2.new(1, 0, 1, 0);
		BackgroundTransparency = 1;
		Visible = false;
	}

	local PaneContainer = Utility.Create'Frame'
	{
		Name = 'PaneContainer';
		Size = UDim2.new(1, 0, 0.786, 0);
		Position = UDim2.new(0,0,0.214,0);
		BackgroundTransparency = 1;
		Parent = HubContainer;
	}

	AppTabDock:SetParent(HubContainer)
	AppTabDock:SetPosition(UDim2.new(0,0,0.132,0))
	local HomeTab = AppTabDock:AddTab(AppTabDockItemModule(Strings:LocalizedString('HomeWord'):upper(), HomePaneModule(PaneContainer)))
	local AvatarTab = AppTabDock:AddTab(AppTabDockItemModule(Strings:LocalizedString('AvatarWord'):upper(), AvatarPaneModule(PaneContainer)))
	local GameTab = AppTabDock:AddTab(AppTabDockItemModule(Strings:LocalizedString('GameWord'):upper(), GamePaneModule(PaneContainer)))
	local SocialTab = AppTabDock:AddTab(AppTabDockItemModule(Strings:LocalizedString('FriendsWord'):upper(), SocialPaneModule(PaneContainer)))
	local StoreTab = AppTabDock:AddTab(AppTabDockItemModule(Strings:LocalizedString('CatalogWord'):upper(), StorePaneModule(PaneContainer)))


	local RobloxLogo = Utility.Create'ImageLabel'
	{
		Name = 'RobloxLogo';
		Size = UDim2.new(0, 232, 0, 56);
		Position = UDim2.new(0,0,0,0);
		BackgroundTransparency = 1;
		Image = 'rbxasset://textures/ui/Shell/Icons/ROBLOXLogoSmall@1080.png';
		Parent = HubContainer;
	}

	-- Positin/Size set after text bounds set
	local BoundHintContainer = Utility.Create'Frame'
	{
		Name = "BoundHintContainer";
		BackgroundTransparency = 1;
		Visible = false;
		Parent = HubContainer;
	}
	local BoundHintText = Utility.Create'TextLabel'
	{
		Name = "BoundHintText";
		BackgroundTransparency = 1;
		Font = GlobalSettings.RegularFont;
		FontSize = GlobalSettings.TitleSize;
		TextColor3 = GlobalSettings.WhiteTextColor;
		TextXAlignment = GlobalSettings.Right;
		Text = "";
		Parent = BoundHintContainer;
	}
	local BoundHintImage = Utility.Create'ImageLabel'
	{
		Name = "BoundHintImage";
		Size = UDim2.new(0, 83, 0, 83);
		BackgroundTransparency = 1;
		Image = 'rbxasset://textures/ui/Shell/ButtonIcons/XButton.png';
		Parent = BoundHintContainer;
	}

	local function setHintText(newText)
		newText = string.upper(newText)
		local textSize = TextService:GetTextSize(newText, 42, BoundHintText.Font, Vector2.new(0, 0))
		BoundHintText.Size = UDim2.new(0, textSize.x, 0, 83)
		BoundHintText.Position = UDim2.new(1, -textSize.x, 0, -4)
		BoundHintText.Text = newText
		BoundHintContainer.Size = UDim2.new(0, textSize.x + BoundHintImage.Size.X.Offset + 8, 0, 83)
		BoundHintContainer.Position = UDim2.new(1, -BoundHintContainer.Size.X.Offset, 1, -BoundHintContainer.Size.Y.Offset)
		BoundHintContainer.Visible = true
	end

	local function SetSelectedTab(newTab)
		AppTabDock:SetSelectedTab(newTab)
	end

	local function onVRChanged()
		local function AddTabIfNotContains(tab)
			if not AppTabDock:ContainsTab(tab) then
				AppTabDock:AddTab(tab)
			end
		end

		if UserInputService.VREnabled then
			local selectedTab = AppTabDock:GetSelectedTab()

			AppTabDock:RemoveTab(HomeTab)
			AppTabDock:RemoveTab(AvatarTab)
			AppTabDock:RemoveTab(SocialTab)
			AppTabDock:RemoveTab(StoreTab)

			if not (selectedTab and AppTabDock:ContainsTab(selectedTab)) then
				local newTab = AppTabDock:GetTab(1)
				if newTab then
					SetSelectedTab(newTab)
				end
			end
		else
			AddTabIfNotContains(HomeTab)
			AddTabIfNotContains(AvatarTab)
			AddTabIfNotContains(GameTab)
			AddTabIfNotContains(SocialTab)
			AddTabIfNotContains(StoreTab)
		end
	end

	local seenXButtonPressed = false
	local function onOpenSettings(actionName, inputState, inputObject)
		if inputState == Enum.UserInputState.Begin then
			seenXButtonPressed = true
		elseif inputState == Enum.UserInputState.End and seenXButtonPressed then
			local settingsScreen = SettingsScreen()
			EventHub:dispatchEvent(EventHub.Notifications["OpenSettingsScreen"], settingsScreen);
		end
	end

	local function onOpenPartyUI(actionName, inputState, inputObject)
		if inputState == Enum.UserInputState.Begin then
			seenXButtonPressed = true
		elseif inputState == Enum.UserInputState.End and seenXButtonPressed then
			if UserSettings().GameSettings:InStudioMode() or game:GetService('UserInputService'):GetPlatform() == Enum.Platform.Windows then
				ScreenManager:OpenScreen(ErrorOverlay(Errors.Test.FeatureNotAvailableInStudio), false)
			else
				local success, result = pcall(function()
					-- PlatformService may not exist in studio
					return PlatformService:PopupPartyUI(inputObject.UserInputType)
				end)
				if not success then
					ScreenManager:OpenScreen(ErrorOverlay(Errors.PlatformError.PopupPartyUI), false)
				end
			end
		end
	end

	local function onSearchGames(actionName, inputState, inputObject)
		if not XboxGameSearchEnabled then
			return
		end

		if inputState == Enum.UserInputState.Begin then
			seenXButtonPressed = true
		elseif inputState == Enum.UserInputState.End and seenXButtonPressed then
			if PlatformService then
				PlatformService:ShowKeyboard(string.upper(Strings:LocalizedString("SearchGamesPhrase")), "", "", Enum.XboxKeyBoardType.Default)
			end
			seenXButtonPressed = false
		end
	end

	local function setHintAction(selectedTab)
		ContextActionService:UnbindCoreAction("OpenHintAction")
		BoundHintContainer.Visible = false
		if selectedTab == HomeTab then
			setHintText(Strings:LocalizedString("SettingsWord"))
			ContextActionService:BindCoreAction("OpenHintAction", onOpenSettings, false, Enum.KeyCode.ButtonX)
		elseif selectedTab == GameTab and XboxGameSearchEnabled then
			setHintText(Strings:LocalizedString("SearchWord"))
			ContextActionService:BindCoreAction("OpenHintAction", onSearchGames, false, Enum.KeyCode.ButtonX)
		elseif selectedTab == SocialTab then
			setHintText(Strings:LocalizedString("StartPartyPhrase"))
			ContextActionService:BindCoreAction("OpenHintAction", onOpenPartyUI, false, Enum.KeyCode.ButtonX)
		end
	end

	local function onSelectedTabChanged(prevSelectedTab, selectedTab)
		local prevSelectedContentPane = prevSelectedTab and prevSelectedTab:GetContentItem()
		if prevSelectedContentPane then
			prevSelectedContentPane:RemoveFocus()
			prevSelectedContentPane:Hide()
		end

		local newSelectedContentPane = selectedTab and selectedTab:GetContentItem()
		if newSelectedContentPane then
			if isShown then
				newSelectedContentPane:Show()
				if isFocused then
					if not AppTabDock:IsFocused() then
						newSelectedContentPane:Focus()
					end
				end
			end
		end
		setHintAction(selectedTab)
	end

	function this:GetName()
		if Utility.ShouldUseVRAppLobby() then
			local currentlySelectedTab = AppTabDock:GetSelectedTab()
			local currentContentPane = currentlySelectedTab and currentlySelectedTab:GetContentItem()
			return currentContentPane and currentContentPane:GetName() or Strings:LocalizedString('HomeWord')
		end

		return lastSelectedContentPane and lastSelectedContentPane:GetName() or Strings:LocalizedString('HomeWord')
	end


	function this:Show()
		isShown = true
		if Utility.ShouldUseVRAppLobby() then
			onVRChanged()
			if not VRChangedConnection then
				VRChangedConnection = UserInputService.Changed:connect(function(prop)
					if prop == 'VREnabled' then
						onVRChanged()
					end
				end)
			end

			if not SelectedTabChangedConnection then
				SelectedTabChangedConnection = AppTabDock.SelectedTabChanged:connect(onSelectedTabChanged)
			end
		end

		HubContainer.Visible = true
		HubContainer.Parent = lastParent

		EventHub:removeEventListener(EventHub.Notifications["NavigateToRobuxScreen"], 'AppHubListenToRobuxScreenSwitch')
		EventHub:addEventListener(EventHub.Notifications["NavigateToRobuxScreen"], 'AppHubListenToRobuxScreenSwitch', function()
			if ScreenManager:ContainsScreen(this) then
				while ScreenManager:GetTopScreen() ~= this and ScreenManager:ContainsScreen(this) do
					ScreenManager:CloseCurrent()
				end
				if ScreenManager:GetTopScreen() == this then
					if AppTabDock:GetSelectedTab() ~= StoreTab then
						SetSelectedTab(StoreTab)
					end
				end
			end
		end)

		local openEquippedDebounce = false
		EventHub:removeEventListener(EventHub.Notifications["NavigateToEquippedAvatar"], 'AppHubListenToAvatarScreenSwitch')
		EventHub:addEventListener(EventHub.Notifications["NavigateToEquippedAvatar"], 'AppHubListenToAvatarScreenSwitch', function()
			if openEquippedDebounce then return end
			openEquippedDebounce = true
			if ScreenManager:ContainsScreen(this) then
				while ScreenManager:GetTopScreen() ~= this and ScreenManager:ContainsScreen(this) do
					ScreenManager:CloseCurrent()
				end
				if ScreenManager:GetTopScreen() == this then
					if AppTabDock:GetSelectedTab() ~= AvatarTab then
						SetSelectedTab(AvatarTab)
					end
				end
			end
			openEquippedDebounce = false
		end)

		if Utility.ShouldUseVRAppLobby() then
			local currentlySelectedTab = AppTabDock:GetSelectedTab()
			local currentContentPane = currentlySelectedTab and currentlySelectedTab:GetContentItem()
			if currentContentPane then
				currentContentPane:Show()
			end
		else
			local currentlySelectedTab = AppTabDock:GetSelectedTab()
			AppTabDock:SetSelectedTab(currentlySelectedTab)
			if lastSelectedContentPane then
				lastSelectedContentPane:Show()
			end
		end
	end

	function this:Hide()
		isShown = false
		if Utility.ShouldUseVRAppLobby() then
			if VRChangedConnection then
				VRChangedConnection:disconnect()
				VRChangedConnection = nil
			end

			if SelectedTabChangedConnection then
				SelectedTabChangedConnection:disconnect()
				SelectedTabChangedConnection = nil
			end
		end

		if not ScreenManager:ContainsScreen(self) then
			EventHub:removeEventListener(EventHub.Notifications["NavigateToRobuxScreen"], 'AppHubListenToRobuxScreenSwitch')
			EventHub:removeEventListener(EventHub.Notifications["NavigateToEquippedAvatar"], 'AppHubListenToAvatarScreenSwitch')
		end
		HubContainer.Visible = false
		HubContainer.Parent = nil

		if Utility.ShouldUseVRAppLobby() then
			local currentlySelectedTab = AppTabDock:GetSelectedTab()
			local currentContentPane = currentlySelectedTab and currentlySelectedTab:GetContentItem()
			if currentContentPane then
				currentContentPane:Hide()
			end
		else
			if lastSelectedContentPane then
				lastSelectedContentPane:Hide()
			end
		end
	end

	function this:Focus()
		isFocused = true
		AppTabDock:ConnectEvents()

		if not Utility.ShouldUseVRAppLobby() then
			ContextActionService:BindCoreAction("CycleTabDock",
				function(actionName, inputState, inputObject)
					if inputState == Enum.UserInputState.End then
						if inputObject.KeyCode == Enum.KeyCode.ButtonL1 then
							local prevTab = AppTabDock:GetPreviousTab()
							if prevTab then
								AppTabDock:SetSelectedTab(prevTab)
							end
						elseif inputObject.KeyCode == Enum.KeyCode.ButtonR1 then
							local nextTab = AppTabDock:GetNextTab()
							if nextTab then
								AppTabDock:SetSelectedTab(nextTab)
							end
						end
					end
				end,
				false,
				Enum.KeyCode.ButtonL1, Enum.KeyCode.ButtonR1)

			local seenBButtonBegin = false
			ContextActionService:BindCoreAction("CloseAppHub",
				function(actionName, inputState, inputObject)
					if inputState == Enum.UserInputState.Begin then
						seenBButtonBegin = true
					elseif inputState == Enum.UserInputState.End then
						if seenBButtonBegin then
							local currentlySelectedTab = AppTabDock:GetSelectedTab()
							if currentlySelectedTab ~= HomeTab then
								AppTabDock:SetSelectedTab(HomeTab)
							end
						end
					end
				end,
				false,
				Enum.KeyCode.ButtonB)

			local function focusTab(tab)
				if tab then
					if lastSelectedContentPane then
						lastSelectedContentPane:Hide()
						lastSelectedContentPane:RemoveFocus()
					end
					local selectedContentPane = tab:GetContentItem()
					if selectedContentPane then
						selectedContentPane:Show()
						if not AppTabDock:IsFocused() then
							AppTabDock:Focus()
							selectedContentPane:Focus()
						end
					end
					lastSelectedContentPane = selectedContentPane

					-- set X actionf
					setHintAction(tab)
				end
			end

			local function onSelectedTabChanged(selectedTab)
				focusTab(selectedTab)
			end
			table.insert(appHubCns, AppTabDock.SelectedTabChanged:connect(onSelectedTabChanged))
		end

		local function onSelectedTabClicked(selectedTab)
			local selectedContentPane = selectedTab and selectedTab:GetContentItem()
			if selectedContentPane then
				selectedContentPane:Focus()
			end
		end
		table.insert(appHubCns, AppTabDock.SelectedTabClicked:connect(onSelectedTabClicked))

		local function onSelectionChanged(prop)
			if Utility.ShouldUseVRAppLobby() then
				return
			end
			if prop == "SelectedObject" then
				local currentSelection = GuiService.SelectedCoreObject
if Utility.IsFastFlagEnabled("XboxFriendsEvents") then
				if currentSelection and lastSelectedContentPane then
					if lastSelectedContentPane.IsAncestorOf == nil or
						lastSelectedContentPane.IsFocused == nil then
						return
					end

					local isPaneAlreadyFocused = lastSelectedContentPane:IsFocused()
					local isSelectionInsidePane = lastSelectedContentPane:IsAncestorOf(currentSelection)

					if isPaneAlreadyFocused and not isSelectionInsidePane then
						lastSelectedContentPane:RemoveFocus()
					elseif not isPaneAlreadyFocused and isSelectionInsidePane then
						lastSelectedContentPane:Focus()
					end
				end
else
				if currentSelection and lastSelectedContentPane then
					-- first condition checks if function exist
					if lastSelectedContentPane.IsFocused and not lastSelectedContentPane:IsFocused() and lastSelectedContentPane.IsAncestorOf then
						if lastSelectedContentPane:IsAncestorOf(currentSelection) then
							-- print("Doing our focus")
							lastSelectedContentPane:Focus()
						else
							-- lastSelectedContentPane:RemoveFocus()
						end
					end
				end
end
			end
		end
		table.insert(appHubCns, GuiService.Changed:connect(onSelectionChanged))

		local function onKeyboardClosed(searchWord)
			searchWord = Utility.SpaceNormalizeString(searchWord)
			if #searchWord > 0 then
				local searchScreen = GameSearchScreen(searchWord)
				searchScreen:SetParent(HubContainer.Parent)
				ScreenManager:OpenScreen(searchScreen)
			end
		end
		if PlatformService and XboxGameSearchEnabled then
			table.insert(appHubCns, PlatformService.KeyboardClosed:connect(onKeyboardClosed))
		end

		-- Go to the default tab (HOME on Xbox or GAMES on VR)
		if AppTabDock:GetSelectedTab() == nil then
			if Utility.ShouldUseVRAppLobby() then
				SetSelectedTab(AppTabDock:GetTab(1))
			else
				AppTabDock:SetSelectedTab(HomeTab)
			end
		end

		if Utility.ShouldUseVRAppLobby() then
			local selectedTab = AppTabDock:GetSelectedTab()
			local selectedContentPane = selectedTab and selectedTab:GetContentItem()
			if selectedContentPane and not AppTabDock:IsFocused() then
				selectedContentPane:Focus()
			end

			setHintAction(selectedTab)
		else
			if lastSelectedContentPane then
				lastSelectedContentPane:Focus()
			end

			setHintAction(AppTabDock:GetSelectedTab())
		end
	end

	function this:RemoveFocus()
		isFocused = false
		AppTabDock:DisconnectEvents()

		if not Utility.ShouldUseVRAppLobby() then
			ContextActionService:UnbindCoreAction("CycleTabDock")
			ContextActionService:UnbindCoreAction("CloseAppHub")

			if lastSelectedContentPane then
				lastSelectedContentPane:RemoveFocus()
			end
		end

		if Utility.ShouldUseVRAppLobby() then
			local selectedTab = AppTabDock:GetSelectedTab()
			local selectedContentPane = selectedTab and selectedTab:GetContentItem()
			if selectedContentPane then
				selectedContentPane:RemoveFocus()
			end
		end

		for k,v in pairs(appHubCns) do
			v:disconnect()
			v = nil
			appHubCns[k] = nil
		end

		ContextActionService:UnbindCoreAction("OpenHintAction")
	end

	function this:SetParent(newParent)
		HubContainer.Parent = newParent
		lastParent = newParent
	end

	local hubID = "AppHub"

	return this
end

return CreateAppHub
