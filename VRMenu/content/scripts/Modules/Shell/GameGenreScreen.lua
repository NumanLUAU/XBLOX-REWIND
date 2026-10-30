--[[
                // GameGenreScreen.lua

                // Creates a GameGenreScreen that is used to navigate games for a
                // selected sort.
]]
local CoreGui = Game:GetService("CoreGui")
local GuiRoot = CoreGui:FindFirstChild("RobloxGui")
local Modules = GuiRoot:FindFirstChild("Modules")
local ShellModules = Modules:FindFirstChild("Shell")
local ContextActionService = game:GetService("ContextActionService")
local GuiService = game:GetService('GuiService')
local PlatformService = nil
pcall(function() PlatformService = game:GetService('PlatformService') end)

local AchievementManager = require(ShellModules:FindFirstChild('AchievementManager'))
local EventHub = require(ShellModules:FindFirstChild('EventHub'))
local GameCollection = require(ShellModules:FindFirstChild('GameCollection'))
local GlobalSettings = require(ShellModules:FindFirstChild('GlobalSettings'))
local ScreenManager = require(ShellModules:FindFirstChild('ScreenManager'))
local SoundManager = require(ShellModules:FindFirstChild('SoundManager'))
local Strings = require(ShellModules:FindFirstChild('LocalizedStrings'))
local Utility = require(ShellModules:FindFirstChild('Utility'))

local BaseCarouselScreen = require(ShellModules:FindFirstChild('BaseCarouselScreen'))
local SideBarModule = require(ShellModules:FindFirstChild('SideBar'))

local function CreateGameGenreScreen(sortName, gameCollection)
    local this = BaseCarouselScreen()

    local currentShownCollection = gameCollection
    local sideBarSelectedCn = nil

    this:SetTitleZIndex(2)
    this:SetTitle(sortName)
    this:LoadGameCollection(gameCollection)

    local sideBarButton = Utility.Create'ImageButton'
    {
        Name = "sideBarButton";
        Size = UDim2.new(0, 450, 0, 75);
        Position = UDim2.new(0, 0, 0, 104);
        BackgroundTransparency = 1;
        BorderSizePixel = 0;
        BackgroundColor3 = GlobalSettings.GreySelectionColor;
        Parent = this.ViewContainer;

        SoundManager:CreateSound('MoveSelection');
    }
    local sideBarImage = Utility.Create'ImageLabel'
    {
        Name = "SideBarImage";
        Size = UDim2.new(0, 44, 0, 44);
        Position = UDim2.new(1, -44 - 12, 0.5, -44/2 + 2);
        BackgroundTransparency = 1;
        Image = 'rbxasset://textures/ui/Shell/Icons/Dropdown02@1080.png';
        Parent = sideBarButton;
    }

    -- selection overrides
    sideBarButton.NextSelectionRight = sideBarButton
    sideBarButton.NextSelectionLeft = sideBarButton

    if Utility.ShouldUseVRAppLobby() then
        sideBarButton.BackgroundTransparency = 0
    else
        sideBarButton.SelectionGained:connect(function()
            Utility.PropertyTweener(sideBarButton, 'BackgroundTransparency', 0, 0, 0, nil, true)
        end)
        sideBarButton.SelectionLost:connect(function()
            Utility.PropertyTweener(sideBarButton, 'BackgroundTransparency', 1, 1, 0, nil, true)
        end)
    end

    -- sidebar functions
    local function canShowFavoritesAsync(collection)
        local favoritesPage = collection:GetSortAsync(0, 1)
        return favoritesPage and favoritesPage.Count > 0
    end
    local function canShowRecentAsync(collection)
        local recentPage = collection:GetSortAsync(0, 1)
        return recentPage and recentPage.Count > 0
    end
    local function canShowUserPlacesAsync(collection)
        local userPlacesPage = collection:GetSortAsync(0, 1)
        return userPlacesPage and userPlacesPage.Count > 0
    end

    local function getSideBarListAsync()
        local sideBarList = {}

        local favoriteCollection = GameCollection:GetUserFavorites()
        if canShowFavoritesAsync(favoriteCollection) then
            table.insert(sideBarList,
                { Name = Strings:LocalizedString("FavoritesSortTitle"), Collection = favoriteCollection })
        end
        local recentCollection = GameCollection:GetUserRecent()
        if canShowRecentAsync(recentCollection) then
            table.insert(sideBarList,
                { Name = Strings:LocalizedString("RecentlyPlayedSortTitle"), Collection = recentCollection })
        end
        table.insert(sideBarList, { Name = Strings:LocalizedString("FeaturedTitle"), Collection = GameCollection:GetSort(GameCollection.DefaultSortId.Featured) })

        -- only add the following sorts if UGC is unlocked
        local hasExplorerAchievement = AchievementManager:HasAchievementAsync(AchievementManager.AchivementId.Explorer)
        if hasExplorerAchievement then
            table.insert(sideBarList, { Name = Strings:LocalizedString("PopularTitle"), Collection = GameCollection:GetSort(GameCollection.DefaultSortId.Popular) })
            table.insert(sideBarList, { Name = Strings:LocalizedString("TopRatedTitle"), Collection = GameCollection:GetSort(GameCollection.DefaultSortId.TopRated) })
            table.insert(sideBarList, { Name = Strings:LocalizedString("TopEarningTitle"), Collection = GameCollection:GetSort(GameCollection.DefaultSortId.TopEarning) })

            local userPlacesCollection = GameCollection:GetUserPlaces()
            if canShowUserPlacesAsync(userPlacesCollection) then
                table.insert(sideBarList,
                    { Name = Strings:LocalizedString("PlayMyPlaceMoreGamesTitle"), Collection = userPlacesCollection })
            end
        end

        return sideBarList
    end

    local sideBar = SideBarModule()
    local currentSideBarItemIndex = 1
    local function populateSideBarAsync()
        sideBar:RemoveAllItems()
        local sideBarList = getSideBarListAsync()
        for i = 1, #sideBarList do
            local sort = sideBarList[i]
            if sort.Collection == currentShownCollection then
                currentSideBarItemIndex = i
            end

            sideBar:AddItem(sort.Name, function()
                if sort.Collection ~= currentShownCollection then
                    currentShownCollection = sort.Collection
                    currentSideBarItemIndex = i
                    this:SetTitle(sort.Name)
                    this:LoadGameCollection(sort.Collection)
                    if this.TransitionTweens then
                        ScreenManager:DefaultCancelFade(this.TransitionTweens)
                        this.TransitionTweens = ScreenManager:DefaultFadeIn(this.Container)
                        ScreenManager:PlayDefaultOpenSound()
                    end
                end
            end)
        end
    end

    local function onUGCUnlocked()
        populateSideBarAsync()
    end

    local baseShow = this.Show
    function this:Show()
        baseShow(self)
        EventHub:addEventListener(EventHub.Notifications["UnlockedUGC"], "UpdateSideBarOnUGCUnlock", onUGCUnlocked)
    end

    local baseHide = this.Hide
    function this:Hide()
        baseHide(self)
        EventHub:removeEventListener(EventHub.Notifications["UnlockedUGC"], "UpdateSideBarOnUGCUnlock")
    end

    local baseGetDefaultSelection = this.GetDefaultSelectionObject
    function this:GetDefaultSelectionObject()
        return baseGetDefaultSelection(self) or sideBarButton
    end

    local baseFocus = this.Focus
    function this:Focus()
        baseFocus(self)
        spawn(function()
            populateSideBarAsync()
        end)

        sideBarSelectedCn = Utility.DisconnectEvent(sideBarSelectedCn)
        sideBarSelectedCn = sideBarButton.MouseButton1Click:connect(function()
            sideBar:SetSelectedObject(currentSideBarItemIndex)
            ScreenManager:OpenScreen(sideBar, false)
        end)
    end

    local baseRemoveFocus = this.RemoveFocus
    function this:RemoveFocus()
        baseRemoveFocus(self)
        sideBarSelectedCn = Utility.DisconnectEvent(sideBarSelectedCn)
    end

    return this
end

return CreateGameGenreScreen
