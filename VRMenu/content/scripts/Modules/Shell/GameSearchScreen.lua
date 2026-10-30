--[[
                // GameSearchScreen.lua

                // Creates a screen for the results of a game search
]]
local CoreGui = Game:GetService("CoreGui")
local GuiRoot = CoreGui:FindFirstChild("RobloxGui")
local Modules = GuiRoot:FindFirstChild("Modules")
local ShellModules = Modules:FindFirstChild("Shell")
local PlatformService = nil
pcall(function() PlatformService = game:GetService('PlatformService') end)
local TextService = game:GetService('TextService')
local ContextActionService = game:GetService("ContextActionService")

local GameCollection = require(ShellModules:FindFirstChild('GameCollection'))
local GlobalSettings = require(ShellModules:FindFirstChild('GlobalSettings'))
local Strings = require(ShellModules:FindFirstChild('LocalizedStrings'))
local Utility = require(ShellModules:FindFirstChild('Utility'))

local BaseCarouselScreen = require(ShellModules:FindFirstChild('BaseCarouselScreen'))

local function CreateGameSearchScreen(searchKeyword)
    local this = BaseCarouselScreen()

    local currentSearchWord = searchKeyword
    local keyboardClosedCn = nil
    local searchGameCollection = GameCollection:GetGameSearchCollection(currentSearchWord)

    local function setTitle(searchWord)
        this:SetTitle(string.format(Strings:LocalizedString("SearchingForPhrase"), searchWord))
    end
    setTitle(currentSearchWord)
    this:LoadGameCollection(searchGameCollection)

     -- search hint
    local SearchActionContainer = Utility.Create'Frame'
    {
        Name = "SearchActionContainer";
        BackgroundTransparency = 1;
        Parent = this.Container;
    }
    local SearchActionText = Utility.Create'TextLabel'
    {
        Name = "SearchActionText";
        BackgroundTransparency = 1;
        Font = GlobalSettings.RegularFont;
        FontSize = GlobalSettings.TitleSize;
        TextColor3 = GlobalSettings.WhiteTextColor;
        TextXAlignment = GlobalSettings.Right;
        Text = string.upper(Strings:LocalizedString("SearchWord"));
        Parent = SearchActionContainer;
    }
    local SearchActionImage = Utility.Create'ImageLabel'
    {
        Name = "SearchActionImage";
        Size = UDim2.new(0, 83, 0, 83);
        BackgroundTransparency = 1;
        Image = 'rbxasset://textures/ui/Shell/ButtonIcons/XButton.png';
        Parent = SearchActionContainer;
    }
    local searchTextSize = TextService:GetTextSize(SearchActionText.Text, 42, SearchActionText.Font, Vector2.new(0, 0))
    SearchActionText.Size = UDim2.new(0, searchTextSize.x, 0, 83)
    SearchActionText.Position = UDim2.new(1, -searchTextSize.x + 4, 0, -4)
    SearchActionContainer.Size = UDim2.new(0, searchTextSize.x + SearchActionImage.Size.X.Offset + 4, 0, 83)
    SearchActionContainer.Position = UDim2.new(1, - SearchActionContainer.Size.X.Offset, 1, -SearchActionContainer.Size.Y.Offset)

    local function onKeyboardClosed(searchWord)
        searchWord = Utility.SpaceNormalizeString(searchWord)
        if #searchWord > 0 and searchWord ~= currentSearchWord then
            currentSearchWord = searchWord
            setTitle(currentSearchWord)
            searchGameCollection = GameCollection:GetGameSearchCollection(currentSearchWord)
            this:LoadGameCollection(searchGameCollection)
        end
    end
    
    local seenYButtonPressed = false
    local function onSearchGames(actionName, inputState, inputObject)
        if inputState == Enum.UserInputState.Begin then
            seenYButtonPressed = true
        elseif inputState == Enum.UserInputState.End and seenYButtonPressed then
            if PlatformService then
                PlatformService:ShowKeyboard(string.upper(Strings:LocalizedString("SearchGamesPhrase")), "", currentSearchWord, Enum.XboxKeyBoardType.Default)
            end
            seenYButtonPressed = false
        end
    end
    
    local baseFocus = this.Focus
    function this:Focus()
        baseFocus(self)
        ContextActionService:BindCoreAction("OpenSearchKeyboard", onSearchGames, false, Enum.KeyCode.ButtonX)
        keyboardClosedConn = Utility.DisconnectEvent(keyboardClosedConn)
        if PlatformService then
            keyboardClosedConn = PlatformService.KeyboardClosed:connect(onKeyboardClosed)
        end
    end
    
    local baseRemoveFocus = this.RemoveFocus
    function this:RemoveFocus()
        baseRemoveFocus(self)
        ContextActionService:UnbindCoreAction("OpenSearchKeyboard")
        keyboardClosedConn = Utility.DisconnectEvent(keyboardClosedConn)
    end

    return this
end

return CreateGameSearchScreen
