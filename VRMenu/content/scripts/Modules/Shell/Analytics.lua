--[[
			// Analytics.lua

			// Fetches analytics data for console platform
]]
local CoreGui = game:GetService("CoreGui")
local GuiRoot = CoreGui:FindFirstChild("RobloxGui")
local Modules = GuiRoot:FindFirstChild("Modules")
local ShellModules = Modules:FindFirstChild("Shell")

local Utility = require(ShellModules:FindFirstChild('Utility'))

--[[ Services ]]--
local AnalyticsService = nil
pcall(function() AnalyticsService = game:GetService('AnalyticsService') end)
local UserInputService = game:GetService('UserInputService')

local IsEventIngestEnabled = Utility.IsFastFlagEnabled('XboxSendEventIngestEvents')

local Analytics = {}

--[[ Helper Functions ]]--
local function setRBXEvent(eventName, additionalArgs)
	local eventContext = nil
	local success, result = pcall(function()
		if UserInputService:GetPlatform() == Enum.Platform.XBoxOne then
			eventContext = "XboxOne"
			eventName = eventName or ""
			additionalArgs = additionalArgs or {}
			AnalyticsService:SetRBXEvent("console", eventContext, eventName, additionalArgs)
		end
	end)

	if not success then
		print("setRBXEvent() failed because", result, "Input: eventContext:", eventContext, "eventName:", eventName)
	end

	return success
end

local function updateHeartbeatObject(additionalArgs)
	local success, result = pcall(function()
		AnalyticsService:UpdateHeartbeatObject(additionalArgs)
	end)

	if not success then
		print("UpdateHeartbeatObject() failed because ", result, "Input: args:", additionalArgs)
	end

	return success
end

local function reportCounter(counterName, amount)
	local success, result = pcall(function()
		if UserInputService:GetPlatform() == Enum.Platform.XBoxOne then
			counterName = counterName or ""
			counterName = "Xbox-"..tostring(counterName)
			amount = amount or 1
			AnalyticsService:ReportCounter(counterName, amount)
		end
	end)

	if not success then
		print("reportCounter() failed because", result, "Input: counterName:", counterName, "amount:", amount)
	end

	return success
end

--[[ Public API ]]--
function Analytics.SetRBXEvent(eventName, additionalArgs)
	if IsEventIngestEnabled then
		setRBXEvent(eventName, additionalArgs)
	end
end

function Analytics.UpdateHeartbeatObject(additionalArgs)
	if IsEventIngestEnabled then
		updateHeartbeatObject(additionalArgs)
	end
end

function Analytics.ReportCounter(counterName, amount)
	reportCounter(counterName, amount)
end

return Analytics